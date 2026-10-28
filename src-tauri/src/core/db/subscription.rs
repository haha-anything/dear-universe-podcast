//! `subscription` 表的读写：增量抓取的缓存头 + 刷新节流。
//!
//! 这张表不存「订阅关系」——订阅就是 `podcast` 表本身（§8.1）。
//! 这里只放「上次抓回来时服务端给的 ETag / Last-Modified」，
//! 下次请求带上，服务端回 304 就能省掉一次解析。

use rusqlite::{params, Connection};

/// 条件请求要带的三个头
#[derive(Debug, Clone, Default)]
pub struct CacheHeaders {
    pub etag: String,
    pub last_modified: String,
    pub last_checked: i64,
}

pub fn get(conn: &Connection, origin: &str) -> Result<CacheHeaders, rusqlite::Error> {
    conn.query_row(
        "SELECT etag, last_modified, last_checked FROM subscription WHERE origin = ?1",
        params![origin],
        |row| {
            Ok(CacheHeaders {
                etag: row.get(0)?,
                last_modified: row.get(1)?,
                last_checked: row.get(2)?,
            })
        },
    )
    // 没有对应行（节目刚订、还没刷过）当作「没有缓存」
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(CacheHeaders::default()),
        other => Err(other),
    })
}

/// 抓完写回：200 时把新的缓存头存下来，304 时只更新 last_checked。
pub fn mark_checked(
    conn: &Connection,
    origin: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    checked_at: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO subscription (origin, etag, last_modified, last_checked)
         VALUES (?1, COALESCE(?2, ''), COALESCE(?3, ''), ?4)
         ON CONFLICT(origin) DO UPDATE SET
            etag          = COALESCE(?2, subscription.etag),
            last_modified = COALESCE(?3, subscription.last_modified),
            last_checked  = ?4",
        params![origin, etag, last_modified, checked_at],
    )?;
    Ok(())
}

/// 该刷的节目：`last_checked` 早于 `before` 的，按最久没刷的排前面
pub fn list_stale(
    conn: &Connection,
    before: i64,
    limit: i64,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT origin FROM subscription
          WHERE last_checked < ?1
          ORDER BY last_checked
          LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![before, limit], |row| row.get(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 未计数更新。刷新里新集的数量写在这里，侧边栏的角标读它。
pub fn set_new_count(conn: &Connection, origin: &str, new_count: i64) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE subscription SET new_count = ?2 WHERE origin = ?1",
        params![origin, new_count],
    )?;
    Ok(())
}
