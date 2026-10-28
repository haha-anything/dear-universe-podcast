//! SQLite 连接管理 + 迁移。
//!
//! AGENTS.md：所有 SQL 只出现在 `core/db/` 这个模块及其子模块里，
//! 业务层和命令层一律看不见 SQL 字符串。

pub mod episode;
pub mod history;
pub mod play_state;
pub mod podcast;
pub mod queue;
pub mod session;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OpenFlags};

use crate::error::AppError;

/// 迁移脚本，按数组顺序执行，下标 + 1 就是 schema_version。
/// 用 include_str! 编进二进制，安装包里不用额外带文件。
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_init.sql",
        include_str!("../../../migrations/001_init.sql"),
    ),
    (
        "002_queue_order.sql",
        include_str!("../../../migrations/002_queue_order.sql"),
    ),
];

/// 应用自己的库。整进程一份，通过 Tauri managed state 注入命令层。
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开（必要时创建）数据目录下的 `dear-universe.db` 并跑到最新迁移。
    pub fn open_in(data_dir: &Path) -> Result<Self, AppError> {
        std::fs::create_dir_all(data_dir).map_err(|e| AppError::Io(e.to_string()))?;

        let path = data_dir.join("dear-universe.db");
        let mut conn = connect(&path)?;
        migrate(&mut conn)?;

        tracing::info!(path = %path.display(), "数据库就绪");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 直接拿连接。只读查询用这个；需要原子性的多处写入请用 [`Self::write`]。
    pub fn read(&self) -> Result<MutexGuard<'_, Connection>, AppError> {
        self.conn.lock().map_err(|e| AppError::DbLock(e.to_string()))
    }

    /// 把一批写操作包进单个事务：要么全成功，要么全回滚。
    ///
    /// 旧库导入（一次写几千行）必须走这里 —— 写到一半失败留下一半数据是灾难。
    pub fn write<T>(
        &self,
        body: impl FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    ) -> Result<T, AppError> {
        let mut guard = self.read()?;
        let tx = guard.transaction()?;
        let out = body(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

fn connect(path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(path)?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

/// 每个连接都必须开的 PRAGMA。
///
/// `foreign_keys` 默认是 **关** 的，不开的话 `ON DELETE CASCADE` 全是摆设 ——
/// 参考实现就是没开，攒了 1065 条孤儿进度记录（任务书 §8.3）。
fn apply_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    // journal_mode / synchronous 只能在事务外设置，别挪进 migrate() 里
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 3_000)?;
    Ok(())
}

/// 只读连接。用 URI 参数 `mode=ro`，从根上杜绝误写（任务书 §15.1）。
///
/// `immutable` 只在确认没有 -wal 伴生文件时才加：告诉 SQLite「这文件不会变」之后
/// 它就不再读 WAL，会漏掉还没落盘的提交。
pub fn connect_read_only(path: &Path) -> Result<Connection, AppError> {
    if !path.exists() {
        return Err(AppError::FileMissing(path.display().to_string()));
    }

    let has_wal = path.with_extension("db-wal").exists();
    let mut uri = String::from("file:");
    uri.push_str(&encode_uri_path(path));
    uri.push_str("?mode=ro");
    if !has_wal {
        uri.push_str("&immutable=1");
    }

    let conn = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

/// `file:` URI 的路径部分：反斜杠换成正斜杠，再转义 URI 里的特殊字符
fn encode_uri_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('%', "%25") // 必须先转义百分号，否则后面两步会把 % 再吃掉
        .replace('?', "%3F")
        .replace('#', "%23")
}

fn migrate(conn: &mut Connection) -> Result<(), AppError> {
    // 首次运行时 schema_version 还不存在，查不出来当作 0
    let applied: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for (index, (name, sql)) in MIGRATIONS.iter().enumerate() {
        #[allow(clippy::cast_possible_wrap)]
        let version = index as i64 + 1;
        if version <= applied {
            continue;
        }

        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT OR REPLACE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![version, now_ms()],
        )?;
        tx.commit()?;

        tracing::info!(migration = name, version, "迁移已应用");
    }

    Ok(())
}

/// 毫秒时间戳。整个项目的「时间」统一用 epoch 毫秒，不要混秒。
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
