use tauri::State;

use crate::{
    core::{
        db::{self, Database},
        download::{self, Downloader},
        http::HttpClient,
    },
    model::DownloadTaskDto,
};

#[tauri::command]
pub fn download_list(db: State<'_, Database>) -> Result<Vec<DownloadTaskDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::download::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_start(
    episode_id: String,
    app: tauri::AppHandle,
    http: State<'_, HttpClient>,
    db: State<'_, Database>,
    manager: State<'_, Downloader>,
    shared: State<'_, db::Shared>,
) -> Result<(), String> {
    let (url, origin) = {
        let conn = db.read().map_err(|e| e.to_string())?;
        let episode = db::episode::get(&conn, &episode_id).map_err(|e| e.to_string())?;
        let episode = episode.ok_or_else(|| "这一集不在库里".to_string())?;
        (episode.audio_url, episode.origin)
    };

    // 已经下完的不重复下
    {
        let conn = db.read().map_err(|e| e.to_string())?;
        if db::download::completed_file(&conn, &episode_id)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Ok(());
        }
    }

    let client = (*http).inner().clone();
    download::start(
        (*manager).clone(),
        shared.0.clone(),
        app,
        client,
        episode_id,
        origin,
        url,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn download_pause(episode_id: String, manager: State<'_, Downloader>) -> Result<(), String> {
    manager.pause(&episode_id);
    Ok(())
}

#[tauri::command]
pub fn download_resume(episode_id: String, manager: State<'_, Downloader>) -> Result<(), String> {
    manager.resume(&episode_id);
    Ok(())
}

#[tauri::command]
pub fn download_cancel(
    episode_id: String,
    manager: State<'_, Downloader>,
    db: State<'_, Database>,
) -> Result<(), String> {
    manager
        .remove(&db, &episode_id)
        .map_err(|e| e.to_string())
}

/// 已完成下载的本地文件完整路径；没下完返回 null。
/// 前端拿它走 `convertFileSrc` 播本地文件，省流量也能离线听。
#[tauri::command]
pub fn download_path(
    episode_id: String,
    manager: State<'_, Downloader>,
    db: State<'_, Database>,
) -> Result<Option<String>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    let name = db::download::completed_file(&conn, &episode_id).map_err(|e| e.to_string())?;

    Ok(name.map(|n| manager.dir().join(n).to_string_lossy().to_string()))
}

/// 限速，字节/秒。传 0 表示不限。
#[tauri::command]
pub fn download_set_limit(bytes_per_sec: u64, manager: State<'_, Downloader>) -> u64 {
    manager.set_limit(bytes_per_sec)
}
