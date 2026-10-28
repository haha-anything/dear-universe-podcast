//! `play_state` 表的读写。
//!
//! 进度来自 `timeupdate`（约 4Hz），所以 save 会被高频调用：
//! 节流交给前端做（每 5 秒一次 + 暂停/切集时 flush），这里只保证写入正确。

use rusqlite::{params, Connection, OptionalExtension};

use super::now_ms;
use crate::model::PlayStateDto;

/// 听完整 (>98%) 就置位。这是个**粘性位**：重听也不回退（§8.7）。
const FINISH_RATIO: f64 = 0.98;

pub fn save(
    conn: &Connection,
    episode_id: &str,
    position_ms: i64,
    duration_ms: i64,
) -> Result<(), rusqlite::Error> {
    let already_finished: i64 = conn
        .query_row(
            "SELECT COALESCE(finished, 0) FROM play_state WHERE episode_id = ?1",
            params![episode_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let finished = already_finished == 1 || is_finished(position_ms, duration_ms);

    conn.execute(
        "INSERT INTO play_state (episode_id, position_ms, duration_ms, finished, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(episode_id) DO UPDATE SET
            position_ms = excluded.position_ms,
            duration_ms = CASE WHEN excluded.duration_ms > 0
                               THEN excluded.duration_ms
                               ELSE play_state.duration_ms END,
            finished    = ?4,
            updated_at  = excluded.updated_at",
        params![
            episode_id,
            position_ms,
            duration_ms,
            i64::from(finished),
            now_ms()
        ],
    )?;

    Ok(())
}

/// 写一条指定 `updated_at` 的进度，冲突时保留已有值不动。
///
/// 旧库导入专用：一方面要还原原始时间戳，另一方面遵循「新库进度优先」（§15.4），
/// 用户已经在用的进度不能被老数据盖回去。
pub fn insert_if_absent(
    conn: &Connection,
    episode_id: &str,
    position_ms: i64,
    duration_ms: i64,
    finished: bool,
    updated_at: i64,
) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "INSERT INTO play_state (episode_id, position_ms, duration_ms, finished, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(episode_id) DO NOTHING",
        params![
            episode_id,
            position_ms,
            duration_ms,
            i64::from(finished),
            updated_at
        ],
    )
}

pub fn get(conn: &Connection, episode_id: &str) -> Result<Option<PlayStateDto>, rusqlite::Error> {
    conn.query_row(
        "SELECT episode_id, position_ms, duration_ms, finished, updated_at
         FROM play_state WHERE episode_id = ?1",
        params![episode_id],
        |row| {
            Ok(PlayStateDto {
                episode_id: row.get(0)?,
                position_sec: row.get::<_, i64>(1)? / 1000,
                duration_sec: row.get::<_, i64>(2)? / 1000,
                finished: row.get::<_, i64>(3)? != 0,
                updated_at: row.get(4)?,
            })
        },
    )
    .optional()
}

fn is_finished(position_ms: i64, duration_ms: i64) -> bool {
    if duration_ms <= 0 || position_ms <= 0 {
        return false;
    }
    #[allow(clippy::cast_precision_loss)]
    let ratio = position_ms as f64 / duration_ms as f64;
    ratio > FINISH_RATIO
}
