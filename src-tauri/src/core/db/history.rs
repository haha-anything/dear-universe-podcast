//! `history` 表的读写。一行 = 一次播放记录。
//!
//! 和 `play_state` 的区别：那条是「听到哪了」（一集一条，会被覆盖），
//! 这条是「什么时候听过」（一集可以有很多条，只增不减）。

use rusqlite::{params, Connection};

use super::episode::COLS;
use crate::model::HistoryEntryDto;

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

/// 播放历史，最近的在前。单集被删了的行读不出来（JOIN 不上），
/// 这是有意的 —— 历史是给用户看的，指向一条不存在的单集没意义。
pub fn list(conn: &Connection, limit: i64, offset: i64) -> Result<Vec<HistoryEntryDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT h.id, h.played_at, {COLS}
         FROM history h
         JOIN episode e ON e.id = h.episode_id
         JOIN podcast p ON p.origin = e.origin
         LEFT JOIN play_state ps ON ps.episode_id = e.id
         ORDER BY h.played_at DESC
         LIMIT ?1 OFFSET ?2"
    ))?;

    let mut rows = stmt.query_map(params![limit, offset], |row| {
        Ok(HistoryEntryDto {
            id: row.get(0)?,
            played_at: row.get(1)?,
            episode: super::episode::row_to_dto(row, 2)?,
        })
    })?;

    let mut out = Vec::new();
    while let Some(row) = rows.next() {
        out.push(row?);
    }
    Ok(out)
}

pub fn remove(conn: &Connection, id: i64) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM history WHERE id = ?1", [id])
}

pub fn clear(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM history", [])
}
