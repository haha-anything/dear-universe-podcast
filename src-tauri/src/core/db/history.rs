//! `history` 表的读写。一行 = 一次播放记录。

use rusqlite::{params, Connection};

pub fn insert(
    conn: &Connection,
    episode_id: &str,
    origin: &str,
    played_at: i64,
) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "INSERT INTO history (episode_id, origin, played_at) VALUES (?1, ?2, ?3)",
        params![episode_id, origin, played_at],
    )
}
