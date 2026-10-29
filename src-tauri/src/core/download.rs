//! 下载：Range 续传 + 令牌桶限速（§13）。
//!
//! 两条硬要求：
//!   1. **断网重连能续传** —— 已有文件就带 `Range: bytes=<received>-`，
//!      服务端回 206 就追加写；回 200 说明它不支持续传，删文件重头下。
//!   2. **限速** —— 令牌桶，默认不限速（0），用户可以在设置里压低。
//!
//! 任务状态写 `download_task` 表，进度通过事件推给前端。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use reqwest::header::RANGE;
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

use crate::{
    core::db::{self, Database},
    error::AppError,
    model::DownloadProgress,
};

/// 进度事件的推送间隔（§13：每 500ms）
const EMIT_INTERVAL_MS: u64 = 500;

/// 最多同时下几个。和刷新同理，抢带宽反而都慢。
const MAX_CONCURRENT: usize = 2;

struct Task {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

/// 下载管理器。做成可克隆的句柄（内部全是 Arc），
/// 这样命令层 `(*state).clone()` 就能把一份所有权交给 tokio 任务。
#[derive(Clone)]
pub struct Downloader {
    inner: Arc<Inner>,
}

struct Inner {
    dir: PathBuf,
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    /// 限速，字节/秒。0 = 不限
    limit: Arc<AtomicU64>,
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl Downloader {
    pub fn new(data_dir: &std::path::Path) -> Result<Self, AppError> {
        let dir = data_dir.join("downloads");
        std::fs::create_dir_all(&dir).map_err(|e| AppError::Io(e.to_string()))?;

        Ok(Self {
            inner: Arc::new(Inner {
                dir,
                tasks: Arc::new(Mutex::new(HashMap::new())),
                limit: Arc::new(AtomicU64::new(0)),
                semaphore: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT)),
            }),
        })
    }

    pub fn set_limit(&self, bytes_per_sec: u64) -> u64 {
        self.inner.limit.store(bytes_per_sec, Ordering::Relaxed);
        bytes_per_sec
    }

    pub fn limit(&self) -> u64 {
        self.inner.limit.load(Ordering::Relaxed)
    }

    /// 下载目录，给前端算本地路径用
    pub fn dir(&self) -> &std::path::Path {
        &self.inner.dir
    }

    pub fn pause(&self, episode_id: &str) -> bool {
        let guard = self.inner.tasks.lock().ok();
        guard
            .and_then(|t| t.get(episode_id).map(|task| task.pause.store(true, Ordering::Relaxed)))
            .is_some()
    }

    pub fn resume(&self, episode_id: &str) -> bool {
        let guard = self.inner.tasks.lock().ok();
        guard
            .and_then(|t| t.get(episode_id).map(|task| task.pause.store(false, Ordering::Relaxed)))
            .is_some()
    }

    pub fn cancel(&self, episode_id: &str) -> bool {
        let guard = self.inner.tasks.lock().ok();
        guard
            .and_then(|t| t.get(episode_id).map(|task| task.cancel.store(true, Ordering::Relaxed)))
            .is_some()
    }

    pub fn is_active(&self, episode_id: &str) -> bool {
        self.inner
            .tasks
            .lock()
            .map(|t| t.contains_key(episode_id))
            .unwrap_or(false)
    }

    /// 删掉文件并清掉任务记录
    pub fn remove(&self, db: &Database, episode_id: &str) -> Result<(), AppError> {
        self.cancel(episode_id);

        if let Ok(conn) = db.read() {
            if let Ok(Some(name)) = db::download::get(&conn, episode_id).map(|t| t.and_then(|t| t.file_name)) {
                let path = self.inner.dir.join(&name);
                if path.exists() {
                    std::fs::remove_file(path).map_err(|e| AppError::Io(e.to_string()))?;
                }
            }
            db::download::remove(&conn, episode_id)?;
        }

        if let Ok(mut tasks) = self.inner.tasks.lock() {
            tasks.remove(episode_id);
        }
        Ok(())
    }
}

/// 启动一次下载。已经在跑的会被忽略。
#[allow(clippy::too_many_arguments)]
pub async fn start(
    downloader: Downloader,
    db: Arc<Database>,
    app: tauri::AppHandle,
    client: reqwest::Client,
    episode_id: String,
    origin: String,
    url: String,
) -> Result<(), AppError> {
    if downloader.is_active(&episode_id) {
        return Ok(());
    }

    let file_name = file_name_for(&episode_id);
    {
        let conn = db.read()?;
        db::download::upsert(&conn, &episode_id, &origin, &file_name, "downloading")?;
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let pause = Arc::new(AtomicBool::new(false));
    if let Ok(mut tasks) = downloader.inner.tasks.lock() {
        tasks.insert(
            episode_id.clone(),
            Task {
                cancel: cancel.clone(),
                pause: pause.clone(),
            },
        );
    }

    let semaphore = downloader.inner.semaphore.clone();
    let dir: PathBuf = downloader.inner.dir.clone();
    let tasks = downloader.inner.tasks.clone();
    let limit = downloader.inner.limit.clone();
    let id = episode_id.clone();

    tokio::spawn(async move {
        let result = run(
            &dir, &client, &url, &file_name, &id, &db, &app, &limit, &cancel, &pause, semaphore,
        )
        .await;

        // 收尾：写状态 + 推事件，无论成功失败都要做
        if let Ok(conn) = db.read() {
            match &result {
                Ok(()) => {
                    let _ = db::download::set_status(&conn, &id, "done", None);
                    let _ = app.emit("download-done", serde_json::json!({ "episode_id": id }));
                }
                Err(e) if e.is_cancelled() => {
                    let _ = db::download::set_status(&conn, &id, "paused", None);
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = db::download::set_status(&conn, &id, "failed", Some(&msg));
                    let _ = app.emit(
                        "download-failed",
                        serde_json::json!({ "episode_id": id, "message": msg }),
                    );
                }
            }
        }

        if let Ok(mut tasks) = tasks.lock() {
            tasks.remove(&id);
        }
    });

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run(
    dir: &std::path::Path,
    client: &reqwest::Client,
    url: &str,
    file_name: &str,
    episode_id: &str,
    db: &Database,
    app: &tauri::AppHandle,
    limit: &AtomicU64,
    cancel: &AtomicBool,
    pause: &AtomicBool,
    semaphore: Arc<tokio::sync::Semaphore>,
) -> Result<(), DownloadError> {
    let _permit = semaphore
        .acquire()
        .await
        .map_err(|_| DownloadError::Cancelled)?;

    let path = dir.join(file_name);
    let mut received = if path.exists() {
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let mut req = client.get(url);
    if received > 0 {
        req = req.header(RANGE, format!("bytes={received}-"));
    }

    let resp = req.send().await.map_err(DownloadError::Network)?;
    let status = resp.status();

    // 206 = 支持续传，从断点继续；200 = 不支持，必须从头来
    let append = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if !append {
        if status != reqwest::StatusCode::OK {
            return Err(DownloadError::Http(status.as_u16()));
        }
        received = 0;
    }

    let total = resp
        .content_length()
        .map(|len| len + if append { received } else { 0 })
        .unwrap_or(0);

    let mut file = if append {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await
    } else {
        tokio::fs::File::create(&path).await
    }
    .map_err(DownloadError::Io)?;

    let mut stream = resp.bytes_stream();
    let mut last_emit = std::time::Instant::now();
    let mut since_emit: u64 = 0;
    // 限速可以在下载中途改，所以每次取当前值而不是构造时固定
    let mut bucket = TokenBucket::new(limit.load(Ordering::Relaxed));

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Err(DownloadError::Cancelled);
        }
        // 暂停：不退出循环，隔 200ms 再看一次，这样恢复能接着下
        while pause.load(Ordering::Relaxed) {
            if cancel.load(Ordering::Relaxed) {
                return Err(DownloadError::Cancelled);
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        let chunk = chunk.map_err(DownloadError::Network)?;
        bucket.take(chunk.len() as u64).await;

        file.write_all(&chunk).await.map_err(DownloadError::Io)?;

        received += chunk.len() as u64;
        since_emit += chunk.len() as u64;

        if last_emit.elapsed().as_millis() as u64 >= EMIT_INTERVAL_MS {
            let speed = (since_emit as f64 / last_emit.elapsed().as_secs_f64()) as i64;
            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    episode_id: episode_id.to_string(),
                    received: i64::try_from(received).unwrap_or(i64::MAX),
                    total: i64::try_from(total).unwrap_or(0),
                    speed,
                },
            );
            if let Ok(conn) = db.read() {
                let _ = db::download::set_progress(
                    &conn,
                    episode_id,
                    i64::try_from(received).unwrap_or(i64::MAX),
                    i64::try_from(total).unwrap_or(0),
                );
            }
            last_emit = std::time::Instant::now();
            since_emit = 0;
        }
    }

    file.flush().await.map_err(DownloadError::Io)?;
    Ok(())
}

/// 令牌桶：按当前限速把写入摊平，避免瞬间打满带宽
struct TokenBucket {
    limit: u64,
    tokens: f64,
    last: std::time::Instant,
}

impl TokenBucket {
    fn new(limit: u64) -> Self {
        Self {
            limit,
            tokens: f64::from(u32::MAX),
            last: std::time::Instant::now(),
        }
    }

    async fn take(&mut self, bytes: u64) {
        // 限速可能在下载中途被改，每次都读当前值
        let limit = self.limit;
        if limit == 0 {
            return;
        }

        let now = std::time::Instant::now();
        let refill = now.duration_since(self.last).as_secs_f64() * limit as f64;
        self.tokens = (self.tokens + refill).min(limit as f64 * 2.0);
        self.last = now;

        if self.tokens >= bytes as f64 {
            self.tokens -= bytes as f64;
            return;
        }

        let wait = (bytes as f64 - self.tokens) / limit as f64;
        self.tokens = 0.0;
        tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
        self.last = std::time::Instant::now();
    }
}

/// 文件名由 episode_id 哈希而来：id 里含 `://` 和中文，不能直接当文件名
fn file_name_for(episode_id: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    episode_id.hash(&mut hasher);
    let digest = hasher.finish();

    // 扩展名从原始 URL 里猜，猜不到就用 .audio
    let ext = episode_id
        .rsplit('.')
        .next()
        .filter(|e| e.len() <= 4 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("audio");

    format!("{digest:016x}.{ext}")
}

#[derive(Debug)]
pub enum DownloadError {
    Network(reqwest::Error),
    Http(u16),
    Io(std::io::Error),
    Cancelled,
}

impl DownloadError {
    fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "网络错误：{e}"),
            Self::Http(code) => write!(f, "服务端返回 HTTP {code}"),
            Self::Io(e) => write!(f, "写文件失败：{e}"),
            Self::Cancelled => write!(f, "已取消"),
        }
    }
}
