//! `queue_item` 表的读写。
//!
//! `episode_id` 允许为空：用户会往队列里塞一条不是任何播客单集的裸音频 URL
//! （参考实现里实测存在，见 §8.5），这类项靠 `raw_url` 兜住。
//!
//! 顺序用 `sort_order`（002 迁移加的），不用自增 id —— 拖动排序要改顺序，
//! 而改主键会让 CASCADE 把关联的进度/收藏连带删掉。

use rusqlite::{params, Connection};

use crate::model::QueueItemDto;

const COLS: &str = "q.id, q.episode_id, q.raw_url, q.title, q.added_at, \
                    e.title, e.audio_url, e.image_url, e.duration_ms, e.origin, \
                    COALESCE(NULLIF(p.custom_title, ''), p.title)";

/// 追加到队尾。`added_at` 由调用方给，保证同毫秒内插入的顺序也稳定。
pub fn insert_at(
    conn: &Connection,
    episode_id: Option<&str>,
    raw_url: Option<&str>,
    title: &str,
    added_at: i64,
) -> Result<usize, rusqlite::Error> {
    let next: i64 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM queue_item", [], |r| {
            r.get(0)
        })
        .unwrap_or(1);

    conn.execute(
        "INSERT INTO queue_item (episode_id, raw_url, title, added_at, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![episode_id, raw_url, title, added_at, next],
    )
}

/// 队列全量。裸 URL 项没有 episode 可 JOIN，所以用 LEFT JOIN 并允许 episode 侧为空。
pub fn list(conn: &Connection) -> Result<Vec<QueueItemDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM queue_item q
         LEFT JOIN episode e ON e.id = q.episode_id
         LEFT JOIN podcast p ON p.origin = e.origin
         ORDER BY q.sort_order, q.id"
    ))?;

    let mut rows = stmt.query_map([], to_dto)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next() {
        out.push(row?);
    }
    Ok(out)
}

/// 队列里第一项（用于「播完自动接上下一集」）
pub fn first(conn: &Connection) -> Result<Option<QueueItemDto>, rusqlite::Error> {
    list(conn).map(|mut v| v.drain(..).next())
}

/// 指定项的下一项；没有就返回 None（队尾）
pub fn next_after(conn: &Connection, item_id: i64) -> Result<Option<QueueItemDto>, rusqlite::Error> {
    let current: Option<i64> = conn
        .query_row("SELECT sort_order FROM queue_item WHERE id = ?1", [item_id], |r| r.get(0))
        .ok();

    let Some(order) = current else {
        // 不在队列里（比如直接从列表点播的），那就从头开始
        return first(conn);
    };

    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM queue_item q
         LEFT JOIN episode e ON e.id = q.episode_id
         LEFT JOIN podcast p ON p.origin = e.origin
         WHERE q.sort_order > ?1
         ORDER BY q.sort_order, q.id
         LIMIT 1"
    ))?;

    stmt.query_row([order], to_dto).optional_row()
}

pub fn remove(conn: &Connection, id: i64) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM queue_item WHERE id = ?1", [id])
}

/// 按单集 id 移除。播完自动出队时用它 —— 用户可能是从列表点播的，
/// 手里只有 episode_id。
pub fn remove_episode(conn: &Connection, episode_id: &str) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM queue_item WHERE episode_id = ?1", [episode_id])
}

pub fn clear(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM queue_item", [])
}

/// 拖动排序：给的 id 序列就是新顺序，下标即 sort_order。
/// 单事务内完成，中途失败不会留下排了一半的队列。
pub fn reorder(conn: &Connection, ids: &[i64]) -> Result<usize, rusqlite::Error> {
    let mut written = 0;
    for (index, id) in ids.iter().enumerate() {
        #[allow(clippy::cast_possible_wrap)]
        let order = index as i64;
        written += conn.execute(
            "UPDATE queue_item SET sort_order = ?1 WHERE id = ?2",
            params![order, id],
        )?;
    }
    Ok(written)
}

/// `query_row` 的「没有匹配行」是 Err(QueryReturnedNoRows)，这里把它折成 None。
trait OptionalRow<T> {
    fn optional_row(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalRow<T> for rusqlite::Result<T> {
    fn optional_row(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

fn to_dto(row: &rusqlite::Row<'_>) -> Result<QueueItemDto, rusqlite::Error> {
    let episode_title: Option<String> = row.get(5)?;
    let duration_ms: Option<i64> = row.get(8)?;

    Ok(QueueItemDto {
        id: row.get(0)?,
        episode_id: row.get(1)?,
        // 标题优先用单集表里的最新值，裸 URL 项才退回队列自带的 title
        title: episode_title
            .or_else(|| row.get::<_, Option<String>>(3).ok().flatten())
            .unwrap_or_default(),
        audio_url: row
            .get::<_, Option<String>>(6)?
            .or_else(|| row.get::<_, Option<String>>(2).ok().flatten()),
        image_url: row.get(7)?,
        duration_sec: duration_ms.and_then(crate::core::db::episode::ms_to_sec),
        podcast_title: row.get(10)?,
    })
}
