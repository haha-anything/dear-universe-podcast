//! `queue_item` 表的读写。
//!
//! `episode_id` 允许为空：用户会往队列里塞一条不是任何播客单集的裸音频 URL
//! （参考实现里实测存在，见 §8.5），这类项靠 `raw_url` 兜住。

use rusqlite::{params, Connection};

/// 按队列顺序追加一项。`added_at` 由调用方给，保证顺序稳定。
pub fn insert_at(
    conn: &Connection,
    episode_id: Option<&str>,
    raw_url: Option<&str>,
    title: &str,
    added_at: i64,
) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "INSERT INTO queue_item (episode_id, raw_url, title, added_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![episode_id, raw_url, title, added_at],
    )
}
