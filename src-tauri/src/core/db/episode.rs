//! `episode` 表的读写。
//!
//! 主键是 `{feedUrl}:{audioUrl}`（§8.4），刷新去重用 `guid`。
//! **注意 insert 不能用 REPLACE**：episode 是父表，`REPLACE` 先删后插，
//! 开着 foreign_keys 会把这条单集的播放进度、收藏、队列项一起带走。

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::EpisodeDto;

/// 单集的列。历史 / 收藏要复用这一整套（连同 JOIN），所以对外可见。
pub(crate) const COLS: &str =
    "e.id, e.guid, e.origin, e.title, e.description, e.audio_url, e.image_url, \
     e.pub_date, e.duration_ms, \
     COALESCE(NULLIF(p.custom_title, ''), p.title), \
     ps.position_ms, COALESCE(ps.finished, 0), \
     p.skip_intro_sec, p.skip_outro_sec";

/// 从 `offset` 开始按 [`COLS`] 的顺序读一条单集。
/// 历史、收藏这类查询会把自己的列放在前面，所以偏移量是参数。
pub fn row_to_dto(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> Result<EpisodeDto, rusqlite::Error> {
    let position_ms: Option<i64> = row.get(offset + 10)?;

    Ok(EpisodeDto {
        id: row.get(offset)?,
        guid: row.get(offset + 1)?,
        origin: row.get(offset + 2)?,
        title: row.get(offset + 3)?,
        description: row.get(offset + 4)?,
        audio_url: row.get(offset + 5)?,
        image_url: row.get(offset + 6)?,
        published_ms: match row.get::<_, i64>(offset + 7)? {
            0 => None,
            ms => Some(ms),
        },
        duration_sec: ms_to_sec(row.get(offset + 8)?),
        podcast_title: row.get(offset + 9)?,
        position_sec: position_ms.and_then(ms_to_sec),
        finished: row.get::<_, i64>(offset + 11)? != 0,
        skip_intro_sec: row.get(offset + 12)?,
        skip_outro_sec: row.get(offset + 13)?,
    })
}

/// 批量入库。`is_new` 是 false 表示「首次抓取当基线」，不冒几百条未读（§13）。
pub fn insert_many(
    conn: &Connection,
    episodes: &[EpisodeDto],
    is_new: bool,
) -> Result<usize, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "INSERT INTO episode (
            id, guid, origin, title, description,
            audio_url, image_url, pub_date, duration_ms, is_new
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            guid        = excluded.guid,
            title       = excluded.title,
            description = excluded.description,
            image_url   = excluded.image_url,
            pub_date    = excluded.pub_date,
            duration_ms = CASE WHEN excluded.duration_ms > 0
                               THEN excluded.duration_ms
                               ELSE episode.duration_ms END",
    )?;

    let mut written = 0;
    for ep in episodes {
        written += stmt.execute(params![
            ep.id,
            ep.guid,
            ep.origin,
            ep.title,
            ep.description,
            ep.audio_url,
            ep.image_url,
            ep.published_ms.unwrap_or(0),
            sec_to_ms(ep.duration_sec),
            i64::from(is_new),
        ])?;
    }

    Ok(written)
}

/// 某个节目的单集，按发布时间倒序
pub fn list_by_origin(
    conn: &Connection,
    origin: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<EpisodeDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM episode e
         JOIN podcast p ON p.origin = e.origin
         LEFT JOIN play_state ps ON ps.episode_id = e.id
         WHERE e.origin = ?1
         ORDER BY e.pub_date DESC
         LIMIT ?2 OFFSET ?3"
    ))?;

    // 写成 while-let 而不是 rows.collect()：collect 会把对 stmt 的借用
    // 拖到函数返回之后，编译器不认（stmt 先于返回值被 drop）。
    let mut rows = stmt.query_map(params![origin, limit, offset], to_dto)?;
    let mut out = Vec::with_capacity(usize::try_from(limit).unwrap_or(64).min(1024));
    while let Some(row) = rows.next() {
        out.push(row?);
    }

    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<EpisodeDto>, rusqlite::Error> {
    conn.query_row(
        &format!(
            "SELECT {COLS} FROM episode e
             JOIN podcast p ON p.origin = e.origin
             LEFT JOIN play_state ps ON ps.episode_id = e.id
             WHERE e.id = ?1"
        ),
        params![id],
        to_dto,
    )
    .optional()
}

/// 这个节目已有的 guid 集合。刷新时用它判断哪些是新的（§13 按 guid 比对）。
pub fn existing_guids(
    conn: &Connection,
    origin: &str,
) -> Result<std::collections::HashSet<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT guid FROM episode WHERE origin = ?1 AND guid IS NOT NULL AND guid != ''",
    )?;
    let rows = stmt.query_map(params![origin], |row| row.get(0))?;

    let mut out = std::collections::HashSet::new();
    for row in rows {
        out.insert(row?);
    }
    Ok(out)
}

pub fn count_by_origin(conn: &Connection, origin: &str) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM episode WHERE origin = ?1",
        params![origin],
        |r| r.get(0),
    )
}

/// 「全部标为已播」：把未读标记清掉
pub fn mark_all_played(conn: &Connection, origin: &str) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "UPDATE episode SET is_new = 0 WHERE origin = ?1 AND is_new = 1",
        params![origin],
    )
}

/// 刷新后的清理：每个节目只留最新 `keep` 条。
///
/// 三类**只更新不删除**（§13）：被收藏的、有播放进度的、在队列里的。
/// 删掉它们等于把用户的进度和收藏一起扔了，参考实现就是这么丢数据的。
pub fn prune_beyond(conn: &Connection, origin: &str, keep: i64) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "DELETE FROM episode
          WHERE origin = ?1
            AND id NOT IN (
                SELECT id FROM episode
                 WHERE origin = ?1
                 ORDER BY pub_date DESC
                 LIMIT ?2
            )
            AND id NOT IN (SELECT episode_id FROM favorite)
            AND id NOT IN (SELECT episode_id FROM play_state)
            AND id NOT IN (SELECT episode_id FROM queue_item WHERE episode_id IS NOT NULL)",
        params![origin, keep],
    )
}

/// 毫秒 → 秒。库里 0 表示「没解析到时长」，转成 None 让前端显示 `--:--`。
pub(crate) fn ms_to_sec(ms: i64) -> Option<i64> {
    (ms > 0).then(|| ms / 1000)
}

/// 秒 → 毫秒。这是 §8.2 那个必须 ×1000 的地方。
pub(crate) fn sec_to_ms(sec: Option<i64>) -> i64 {
    sec.unwrap_or(0).saturating_mul(1000)
}

fn to_dto(row: &rusqlite::Row<'_>) -> Result<EpisodeDto, rusqlite::Error> {
    row_to_dto(row, 0)
}
