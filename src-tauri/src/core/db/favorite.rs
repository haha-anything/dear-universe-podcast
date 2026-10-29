//! `favorite` 表的读写。
//!
//! 收藏是**独立表**而不是 episode 上的一个字段（§8.9）：
//! 它需要自己的时间排序，而且要扛住刷新时的清理（收藏的单集永不被 prune）。

use rusqlite::{params, Connection};

use crate::model::FavoriteEntryDto;

use super::episode::COLS;

pub fn add(conn: &Connection, episode_id: &str, added_at: i64) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO favorite (episode_id, added_at) VALUES (?1, ?2)",
        params![episode_id, added_at],
    )
}

pub fn remove(conn: &Connection, episode_id: &str) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM favorite WHERE episode_id = ?1", [episode_id])
}

pub fn has(conn: &Connection, episode_id: &str) -> Result<bool, rusqlite::Error> {
    conn.query_row(
        "SELECT 1 FROM favorite WHERE episode_id = ?1",
        [episode_id],
        |_| Ok(()),
    )
    .optional_bool()
}

pub fn list(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<FavoriteEntryDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT f.episode_id, f.added_at, {COLS}
         FROM favorite f
         JOIN episode e ON e.id = f.episode_id
         JOIN podcast p ON p.origin = e.origin
         LEFT JOIN play_state ps ON ps.episode_id = e.id
         ORDER BY f.added_at DESC
         LIMIT ?1 OFFSET ?2"
    ))?;

    let mut rows = stmt.query_map(params![limit, offset], |row| {
        Ok(FavoriteEntryDto {
            added_at: row.get(1)?,
            episode: super::episode::row_to_dto(row, 2)?,
        })
    })?;

    let mut out = Vec::new();
    while let Some(row) = rows.next() {
        out.push(row?);
    }
    Ok(out)
}

trait OptionalBool {
    fn optional_bool(self) -> rusqlite::Result<bool>;
}

impl<T> OptionalBool for rusqlite::Result<T> {
    fn optional_bool(self) -> rusqlite::Result<bool> {
        match self {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }
}
