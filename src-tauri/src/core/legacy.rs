//! 从旧版 Podara 的 SQLite 库迁移数据过来（任务书 §15）。
//!
//! 五条安全要求的落点：
//!   1. 只读打开      → `db::connect_read_only`（mode=ro，从根上禁写）
//!   2. 先 dry-run    → [`scan`]
//!   3. 单事务写入    → `Database::write`，失败全回滚
//!   4. 目标库非空时  → upsert，且进度用 `ON CONFLICT DO NOTHING`（新库优先）
//!   5. 写报告        → 导入完把 import-report.json 落到数据目录

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::{Connection, OptionalExtension};

use crate::{
    core::db::{self, Database},
    error::AppError,
    model::{
        ImportReport, LegacyCounts, LegacyScanReport, LegacySkip, PodcastDto,
    },
};

/// 这三张表是 Podara 的核心，缺一张就说明拿错了文件
const REQUIRED_TABLES: &[&str] = &["podcast", "podcastEpisode", "podcastEpisodePlayState"];

/// 旧库的默认位置 `%USERPROFILE%\.podara\podara.db`
pub fn default_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".podara").join("podara.db"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// 只读扫一遍老库，算出「哪些会进、哪些会跳」，不写任何东西。
pub fn scan(path: &Path) -> Result<LegacyScanReport, AppError> {
    let report = LegacyScanReport {
        db_path: path.display().to_string(),
        usable: true,
        reason: None,
        source: LegacyCounts::default(),
        plan: LegacyCounts::default(),
        skipped: LegacySkip::default(),
    };

    if !path.exists() {
        return Ok(LegacyScanReport {
            usable: false,
            reason: Some("没有找到这个文件".to_string()),
            ..report
        });
    }

    let src = db::connect_read_only(path)?;

    if let Some(missing) = missing_table(&src)? {
        return Ok(LegacyScanReport {
            usable: false,
            reason: Some(format!("这不是 Podara 的数据库，缺 {missing} 表")),
            ..report
        });
    }

    let source = source_counts(&src)?;
    let plan = LegacyCounts {
        podcasts: source.podcasts,
        episodes: count(
            &src,
            "SELECT COUNT(*) FROM podcastEpisode e
             WHERE EXISTS (SELECT 1 FROM podcast p WHERE p.origin = e.origin)",
        )?,
        play_states: count(
            &src,
            "SELECT COUNT(*) FROM podcastEpisodePlayState s
             WHERE s.positionMs > 0
               AND EXISTS (SELECT 1 FROM podcastEpisode e WHERE e.id = s.episodeId)",
        )?,
        history: count(
            &src,
            "SELECT COUNT(*) FROM podcastHistory h
             WHERE EXISTS (SELECT 1 FROM podcastEpisode e WHERE e.id = h.episodeId)",
        )?,
        queue_items: source.queue_items,
    };

    let skipped = LegacySkip {
        // 位置为 0 的进度没迁的意义，占了老库 5488 行
        empty_play_state: count(
            &src,
            "SELECT COUNT(*) FROM podcastEpisodePlayState WHERE positionMs = 0",
        )?,
        // 单集已经不在库里了 —— 参考实现因为没开外键攒了 1065 条
        orphan_play_state: count(
            &src,
            "SELECT COUNT(*) FROM podcastEpisodePlayState s
             WHERE s.positionMs > 0
               AND NOT EXISTS (SELECT 1 FROM podcastEpisode e WHERE e.id = s.episodeId)",
        )?,
        orphan_history: count(
            &src,
            "SELECT COUNT(*) FROM podcastHistory h
             WHERE NOT EXISTS (SELECT 1 FROM podcastEpisode e WHERE e.id = h.episodeId)",
        )?,
        queue_without_episode: count(
            &src,
            "SELECT COUNT(*) FROM playerQueueItem q
             WHERE q.episodeId IS NULL OR q.episodeId = ''
                OR NOT EXISTS (SELECT 1 FROM podcastEpisode e WHERE e.id = q.episodeId)",
        )?,
        orphan_episode: source.episodes.saturating_sub(plan.episodes),
    };

    Ok(LegacyScanReport {
        source,
        plan,
        skipped,
        ..report
    })
}

/// 真正的导入。整个写库过程在一个事务里，中途任何一步炸了都会回滚。
pub fn import(db: &Database, path: &Path, data_dir: &Path) -> Result<ImportReport, AppError> {
    let started = Instant::now();

    let src = db::connect_read_only(path)?;
    if let Some(missing) = missing_table(&src)? {
        return Err(AppError::InvalidSource(format!("缺 {missing} 表")));
    }

    let (plan, skipped) = db.write(|conn| write_all(conn, &src))?;

    #[allow(clippy::cast_possible_wrap)]
    let elapsed_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);

    let report = ImportReport {
        plan,
        skipped,
        report_path: None,
        elapsed_ms,
    };

    let report = write_report(data_dir, report)?;
    tracing::info!(?report.plan, elapsed_ms, "旧库导入完成");

    Ok(report)
}

/// 按 节目 → 单集 → 进度/历史/队列 的顺序写。顺序不能调：
/// episode 外键指向 podcast，后面的表又外键指向 episode。
fn write_all(
    conn: &Connection,
    src: &Connection,
) -> Result<(LegacyCounts, LegacySkip), rusqlite::Error> {
    let mut plan = LegacyCounts::default();
    let mut skipped = LegacySkip::default();

    // ── 节目 ────────────────────────────────────────────────────────
    let mut origins: HashSet<String> = HashSet::new();
    {
        let mut stmt = src.prepare(
            "SELECT origin, link, title, description, author, imageUrl,
                    languageCode, overrideTitle, skipBeginning, skipEnding
             FROM podcast",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PodcastDto {
                origin: row.get(0)?,
                link: row.get::<_, String>(1).ok(),
                title: row.get(2)?,
                description: row.get::<_, String>(3).ok(),
                author: row.get::<_, String>(4).ok(),
                image_url: row.get::<_, String>(5).ok(),
                language: row.get::<_, String>(6).ok().filter(|s| !s.is_empty()),
                custom_title: row.get::<_, String>(7).unwrap_or_default(),
                skip_intro_sec: row.get(8)?,
                skip_outro_sec: row.get(9)?,
                created_at: 0,
                episode_count: 0,
                new_count: 0,
            })
        })?;

        for row in rows {
            let mut dto = row?;
            dto.created_at = db::now_ms();
            db::podcast::upsert(conn, &dto, "rss")?;
            plan.podcasts += 1;
            origins.insert(dto.origin);
        }
    }

    // ── 单集 ────────────────────────────────────────────────────────
    let mut episodes: HashSet<String> = HashSet::new();
    {
        let mut stmt = src.prepare(
            "SELECT id, guid, origin, link, audioUrl, title, description,
                    imageUrl, pubDate, duration
             FROM podcastEpisode",
        )?;
        let rows = stmt.query_map([], |row| Ok(EpisodeRow {
            id: row.get(0)?,
            guid: row.get::<_, String>(1).ok(),
            origin: row.get(2)?,
            link: row.get::<_, String>(3).ok(),
            audio_url: row.get(4)?,
            title: row.get(5)?,
            description: row.get::<_, String>(6).ok(),
            image_url: row.get::<_, String>(7).ok(),
            pub_date: row.get(8)?,
            duration_sec: row.get(9)?,
        }))?;

        let mut insert = conn.prepare(
            "INSERT INTO episode (
                id, guid, origin, title, description, link,
                audio_url, image_url, pub_date, duration_ms, is_new
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)
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

        for row in rows {
            let ep = row?;
            if !origins.contains(&ep.origin) {
                // 外键要求 origin 必须存在，孤儿单集宁可不导
                skipped.orphan_episode += 1;
                continue;
            }

            insert.execute(rusqlite::params![
                ep.id,
                ep.guid,
                ep.origin,
                ep.title,
                ep.description,
                ep.link,
                ep.audio_url,
                ep.image_url,
                ep.pub_date,
                // 老库的 duration 存的是秒 —— 这是 §8.2 那个必乘 1000 的点
                ep.duration_sec.saturating_mul(1000),
            ])?;

            plan.episodes += 1;
            episodes.insert(ep.id.clone());
        }
    }

    // ── 播放进度（只迁真正听过一点的）────────────────────────────────
    {
        let mut stmt = src.prepare(
            "SELECT episodeId, positionMs, durationMs, finished, lastUpdate
             FROM podcastEpisodePlayState ORDER BY lastUpdate",
        )?;
        let rows = stmt.query_map([], |row| Ok(StateRow {
            episode_id: row.get(0)?,
            position_ms: row.get(1)?,
            duration_ms: row.get(2)?,
            finished: row.get::<_, i64>(3).unwrap_or(0) != 0,
            updated_at: row.get(4)?,
        }))?;

        for row in rows {
            let st = row?;
            if st.position_ms <= 0 {
                skipped.empty_play_state += 1;
                continue;
            }
            if !episodes.contains(&st.episode_id) {
                skipped.orphan_play_state += 1;
                continue;
            }

            let wrote = db::play_state::insert_if_absent(
                conn,
                &st.episode_id,
                st.position_ms,
                st.duration_ms,
                st.finished,
                st.updated_at,
            )?;
            plan.play_states += wrote;
        }
    }

    // ── 历史 ────────────────────────────────────────────────────────
    {
        let mut stmt = src.prepare(
            "SELECT episodeId, origin, timestamp FROM podcastHistory ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| Ok(HistoryRow {
            episode_id: row.get(0)?,
            origin: row.get(1)?,
            played_at: row.get(2)?,
        }))?;

        for row in rows {
            let h = row?;
            if !episodes.contains(&h.episode_id) {
                skipped.orphan_history += 1;
                continue;
            }
            let origin = if origins.contains(&h.origin) {
                h.origin
            } else {
                episode_origin(conn, &h.episode_id)?.unwrap_or_default()
            };
            if origin.is_empty() {
                skipped.orphan_history += 1;
                continue;
            }

            plan.history += db::history::insert(conn, &h.episode_id, &origin, h.played_at)?;
        }
    }

    // ── 队列（按 queueOrder 保序；匹配不上单集的退化成裸 URL）──────────
    {
        let now = db::now_ms();
        let mut stmt = src.prepare(
            "SELECT queueOrder, url, title, episodeId
             FROM playerQueueItem ORDER BY queueOrder",
        )?;
        let rows = stmt.query_map([], |row| Ok(QueueRow {
            order: row.get(0)?,
            url: row.get(1)?,
            title: row.get(2)?,
            episode_id: row.get::<_, String>(3).ok().filter(|s| !s.is_empty()),
        }))?;

        for row in rows {
            let q = row?;
            // added_at 用「同一时刻 + 序号」，保证插入后 id 顺序 = 老队列顺序
            let added_at = now.saturating_add(q.order);

            let wrote = match q.episode_id.filter(|id| episodes.contains(id)) {
                Some(id) => db::queue::insert_at(conn, Some(&id), None, &q.title, added_at)?,
                None => {
                    skipped.queue_without_episode += 1;
                    db::queue::insert_at(conn, None, Some(&q.url), &q.title, added_at)?
                }
            };
            plan.queue_items += wrote;
        }
    }

    Ok((plan, skipped))
}

fn write_report(data_dir: &Path, report: ImportReport) -> Result<ImportReport, AppError> {
    let path = data_dir.join("import-report.json");
    let json = serde_json::to_string_pretty(&report).map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| AppError::Io(e.to_string()))?;

    Ok(ImportReport {
        report_path: Some(path.display().to_string()),
        ..report
    })
}

fn missing_table(src: &Connection) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = src.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
    let present: HashSet<String> = stmt.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?;

    Ok(REQUIRED_TABLES
        .iter()
        .find(|t| !present.contains(**t))
        .map(|t| (*t).to_string()))
}

fn source_counts(src: &Connection) -> Result<LegacyCounts, rusqlite::Error> {
    Ok(LegacyCounts {
        podcasts: count(src, "SELECT COUNT(*) FROM podcast")?,
        episodes: count(src, "SELECT COUNT(*) FROM podcastEpisode")?,
        play_states: count(src, "SELECT COUNT(*) FROM podcastEpisodePlayState")?,
        history: count(src, "SELECT COUNT(*) FROM podcastHistory")?,
        queue_items: count(src, "SELECT COUNT(*) FROM playerQueueItem")?,
    })
}

fn count(src: &Connection, sql: &str) -> Result<usize, rusqlite::Error> {
    src.query_row(sql, [], |row| row.get(0))
}

fn episode_origin(conn: &Connection, id: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row("SELECT origin FROM episode WHERE id = ?1", [id], |r| r.get(0))
        .optional()
}

/// 老库行的临时载体 —— 纯粹是为了让中间那堆 rusqlite 管道好看一点
struct EpisodeRow {
    id: String,
    guid: Option<String>,
    origin: String,
    link: Option<String>,
    audio_url: String,
    title: String,
    description: Option<String>,
    image_url: Option<String>,
    pub_date: i64,
    duration_sec: i64,
}

struct StateRow {
    episode_id: String,
    position_ms: i64,
    duration_ms: i64,
    finished: bool,
    updated_at: i64,
}

struct HistoryRow {
    episode_id: String,
    origin: String,
    played_at: i64,
}

struct QueueRow {
    order: i64,
    url: String,
    title: String,
    episode_id: Option<String>,
}
