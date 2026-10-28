//! `player_session`：单行表，记「上次在听哪一集、听到哪」，用来重启接着播。
//!
//! 注意它存的不是播放进度本身（那是 play_state 的活），
//! 而是「UI 上当前选中了哪一集」这种会话态。

use rusqlite::{params, Connection, OptionalExtension};

use super::now_ms;
use crate::model::SessionDto;

pub fn save(
    conn: &Connection,
    episode_id: Option<&str>,
    position_ms: i64,
    speed: f64,
    volume: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO player_session (id, current_episode, position_ms, speed, volume, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            current_episode = excluded.current_episode,
            position_ms     = excluded.position_ms,
            speed           = excluded.speed,
            volume          = excluded.volume,
            updated_at      = excluded.updated_at",
        params![episode_id, position_ms, speed, volume, now_ms()],
    )?;
    Ok(())
}

pub fn load(conn: &Connection) -> Result<Option<SessionDto>, rusqlite::Error> {
    conn.query_row(
        "SELECT current_episode, position_ms, speed, volume
         FROM player_session WHERE id = 1",
        [],
        |row| {
            Ok(SessionDto {
                current_episode: row.get(0)?,
                position_ms: row.get(1)?,
                speed: row.get(2)?,
                volume: row.get(3)?,
            })
        },
    )
    .optional()
}
