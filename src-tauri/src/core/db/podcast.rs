//! `podcast` 与 `subscription` 两张表的读写。
//!
//! `podcast` 本身就是订阅列表 —— 不存在独立的「订阅关系表」（任务书 §8.1）。
//! 参考实现里那张 `podcastSubscription` 实测 0 行，这次不重演；
//! `subscription` 只放「可选配置 + 增量抓取的缓存头」，两者同生同灭。

use rusqlite::{params, Connection, OptionalExtension};

use super::now_ms;
use crate::model::PodcastDto;

const COLS: &str = "p.origin, p.title, p.custom_title, p.author, p.description, p.image_url, \
                    p.link, p.language, p.skip_intro_sec, p.skip_outro_sec, p.created_at, \
                    (SELECT COUNT(*) FROM episode e WHERE e.origin = p.origin), \
                    (SELECT COUNT(*) FROM episode e WHERE e.origin = p.origin AND e.is_new = 1)";

/// 入库一个节目（已订阅）。重复订阅同一个地址走更新，不报错。
pub fn upsert(conn: &Connection, dto: &PodcastDto, source: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO podcast (
            origin, title, custom_title, author, description, image_url,
            link, language, seed_color, source, skip_intro_sec, skip_outro_sec, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?11, ?12)
         ON CONFLICT(origin) DO UPDATE SET
            title          = excluded.title,
            author         = excluded.author,
            description    = excluded.description,
            image_url      = excluded.image_url,
            link           = excluded.link,
            language       = excluded.language",
        params![
            dto.origin,
            dto.title,
            dto.custom_title,
            dto.author,
            dto.description,
            dto.image_url,
            dto.link,
            dto.language,
            source,
            dto.skip_intro_sec,
            dto.skip_outro_sec,
            dto.created_at,
        ],
    )?;

    // 缓存行跟着建出来，刷新时只更新字段，省得每次 INSERT OR REPLACE 判空
    conn.execute(
        "INSERT OR IGNORE INTO subscription (origin) VALUES (?1)",
        params![dto.origin],
    )?;

    Ok(())
}

pub fn exists(conn: &Connection, origin: &str) -> Result<bool, rusqlite::Error> {
    conn.query_row(
        "SELECT 1 FROM podcast WHERE origin = ?1",
        params![origin],
        |_| Ok(()),
    )
    .optional()
    .map(|row| row.is_some())
}

pub fn get(conn: &Connection, origin: &str) -> Result<Option<PodcastDto>, rusqlite::Error> {
    conn.query_row(
        &format!(
            "SELECT {COLS} FROM podcast p WHERE p.origin = ?1"
        ),
        params![origin],
        to_dto,
    )
    .optional()
}

/// 订阅列表，按展示名排序（`custom_title` 非空时优先）
pub fn list(conn: &Connection) -> Result<Vec<PodcastDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM podcast p
         ORDER BY COALESCE(NULLIF(p.custom_title, ''), p.title) COLLATE NOCASE"
    ))?;

    let rows = stmt.query_map([], to_dto)?;
    rows.collect()
}

/// 退订。episode / play_state / queue_item 都挂在 origin 上，
/// 开了 foreign_keys 之后会由 ON DELETE CASCADE 连带清掉。
pub fn remove(conn: &Connection, origin: &str) -> Result<bool, rusqlite::Error> {
    let n = conn.execute("DELETE FROM podcast WHERE origin = ?1", params![origin])?;
    Ok(n > 0)
}

fn to_dto(row: &rusqlite::Row<'_>) -> Result<PodcastDto, rusqlite::Error> {
    Ok(PodcastDto {
        origin: row.get(0)?,
        title: row.get(1)?,
        custom_title: row.get(2)?,
        author: row.get(3)?,
        description: row.get(4)?,
        image_url: row.get(5)?,
        link: row.get(6)?,
        language: row.get(7)?,
        skip_intro_sec: row.get(8)?,
        skip_outro_sec: row.get(9)?,
        created_at: row.get(10)?,
        episode_count: row.get(11)?,
        new_count: row.get(12)?,
    })
}

/// 新订阅时的 DTO 构造：展示字段之外的东西（计数、时间）由这里补默认值
pub fn new_dto(
    origin: String,
    title: String,
    author: Option<String>,
    description: Option<String>,
    image_url: Option<String>,
    link: Option<String>,
    language: Option<String>,
) -> PodcastDto {
    PodcastDto {
        origin,
        title,
        custom_title: String::new(),
        author,
        description,
        image_url,
        link,
        language,
        skip_intro_sec: 0,
        skip_outro_sec: 0,
        created_at: now_ms(),
        episode_count: 0,
        new_count: 0,
    }
}
