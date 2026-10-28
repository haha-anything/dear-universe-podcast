//! 订阅与刷新。
//!
//! 刷新是整个应用唯一「同时打很多网络请求」的地方，两条铁律：
//!   1. 并发压到 3。20 个频道 3 并发比全并发更快完成（不抢带宽，§17.5）
//!   2. 带 If-None-Match / If-Modified-Since。304 直接跳过解析（§17.4）

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use tokio::sync::Semaphore;

use crate::{
    core::{
        db::{self, Database},
        feed::{self, Fetched},
    },
    error::AppError,
    model::{FeedDto, PodcastDto, RefreshOutcome, RefreshSummary},
};

/// 并发上限（§13）
const MAX_CONCURRENCY: usize = 3;

/// `last_checked` 新于这个间隔的跳过，避免手抖连点把源打爆（§13）
const MIN_INTERVAL_MS: i64 = 10 * 60 * 1000;

/// 每个节目保留的单集数（§13）
pub const KEEP_EPISODES: i64 = 300;

/// 订阅一个 RSS 地址，返回落库后的节目信息。
pub async fn subscribe(
    db: &Database,
    client: &Client,
    feed_url: &str,
) -> Result<PodcastDto, AppError> {
    let parsed = feed::fetch_feed(client, feed_url).await?;
    persist(db, &parsed, "rss")
}

/// 刷单个节目。
///
/// 库里一条单集都没有时算**首次抓取**，全部入库但 `is_new = 0` 当基线——
/// 否则新用户一进来就冒几百条未读（§13）。
pub async fn refresh(
    db: &Database,
    client: &Client,
    origin: &str,
    force: bool,
) -> Result<RefreshOutcome, AppError> {
    let cache = {
        let conn = db.read()?;
        db::subscription::get(&conn, origin)?
    };

    let now = db::now_ms();
    if !force && now - cache.last_checked < MIN_INTERVAL_MS {
        return Ok(RefreshOutcome {
            origin: origin.to_string(),
            new_episodes: 0,
            not_modified: false,
            skipped: true,
        });
    }

    let outcome = match feed::fetch_conditional(
        client,
        origin,
        Some(&cache.etag),
        Some(&cache.last_modified),
    )
    .await
    {
        Ok(Fetched::NotModified) => {
            let conn = db.read()?;
            db::subscription::mark_checked(&conn, origin, None, None, now)?;
            RefreshOutcome {
                origin: origin.to_string(),
                new_episodes: 0,
                not_modified: true,
                skipped: false,
            }
        }
        Ok(Fetched::Changed {
            feed,
            etag,
            last_modified,
        }) => {
            let new_episodes = store(db, &feed, etag.as_deref(), last_modified.as_deref(), now)?;
            RefreshOutcome {
                origin: origin.to_string(),
                new_episodes,
                not_modified: false,
                skipped: false,
            }
        }
        Err(e) => return Err(e),
    };

    Ok(outcome)
}

/// 刷全部。返回汇总；单个源失败不打断整体，失败地址进 `failed`。
///
/// `on_progress(done, total)` 用来往前端推 refresh-progress 事件。
/// core 不认识 Tauri 的事件系统，所以用回调而不是直接 emit。
///
/// 并发靠 `Semaphore(3)` + `JoinSet`。用 Arc<Database> 而不是 &Database 是因为
/// tokio 的任务要求 'static —— rusqlite 的 Connection 不 Sync，但 Mutex<T> 只要
/// T: Send 就 Sync，所以 Arc<Database> 是能安全跨任务共享的。
pub async fn refresh_all(
    db: Arc<Database>,
    client: &Client,
    force: bool,
    on_progress: Arc<dyn Fn(usize, usize) + Send + Sync>,
) -> Result<RefreshSummary, AppError> {
    let origins = {
        let conn = db.read()?;
        db::podcast::list(&conn)?
            .into_iter()
            .map(|p| p.origin)
            .collect::<Vec<_>>()
    };

    let total = origins.len();
    let started = Instant::now();
    tracing::info!(total, "开始刷新全部");

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));
    let done = Arc::new(AtomicUsize::new(0));
    let client = client.clone();

    let mut tasks = tokio::task::JoinSet::new();
    for origin in origins {
        let db = db.clone();
        let client = client.clone();
        let semaphore = semaphore.clone();
        let done = done.clone();
        let on_progress = on_progress.clone();

        tasks.spawn(async move {
            // 信号量在任务里获取：先建任务再加锁，等于把并发交给调度器
            let _permit = semaphore.acquire().await.expect("信号量被关闭");
            let outcome = refresh(&db, &client, &origin, force).await;

            let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
            on_progress(finished, total);

            (origin, outcome)
        });
    }

    let mut total_new = 0u32;
    let mut failed = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((_, Ok(outcome))) => total_new += outcome.new_episodes,
            Ok((origin, Err(e))) => {
                tracing::warn!(origin = %origin, error = %e, "刷新失败");
                failed.push(origin);
            }
            Err(e) => tracing::error!(error = %e, "刷新任务崩溃"),
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    let elapsed_ms = started.elapsed().as_millis() as i64;

    tracing::info!(total_new, failed = failed.len(), elapsed_ms, "刷新完成");

    Ok(RefreshSummary {
        total,
        total_new,
        failed,
        elapsed_ms,
    })
}

/// 把一次抓取的结果写库。
fn persist(db: &Database, parsed: &FeedDto, source: &str) -> Result<PodcastDto, AppError> {
    let podcast = db::podcast::new_dto(
        parsed.feed_url.clone(),
        parsed.title.clone(),
        parsed.author.clone(),
        parsed.description.clone(),
        parsed.image_url.clone(),
        None,
        None,
    );

    db.write(|conn| {
        db::podcast::upsert(conn, &podcast, source)?;
        let written = db::episode::insert_many(conn, &parsed.episodes, false)?;
        tracing::info!(written, origin = %parsed.feed_url, "单集入库");
        Ok(())
    })?;

    // 回读一次是为了带上 insert 之后才算得出来的计数
    let stored = {
        let conn = db.read()?;
        db::podcast::get(&conn, &parsed.feed_url)?
    };

    Ok(stored.unwrap_or(podcast))
}

/// 刷新专用入库：只把没见过的 guid 标成新集，然后按 §13 清理。
fn store(
    db: &Database,
    parsed: &FeedDto,
    etag: Option<&str>,
    last_modified: Option<&str>,
    now: i64,
) -> Result<u32, AppError> {
    let podcast = db::podcast::new_dto(
        parsed.feed_url.clone(),
        parsed.title.clone(),
        parsed.author.clone(),
        parsed.description.clone(),
        parsed.image_url.clone(),
        None,
        None,
    );

    db.write(|conn| {
        db::podcast::upsert(conn, &podcast, "rss")?;

        let known = db::episode::existing_guids(conn, &parsed.feed_url)?;
        let had_any = db::episode::count_by_origin(conn, &parsed.feed_url)? > 0;

        // 首次抓取当基线：全入库，但一条都不标未读
        let is_new = had_any;
        let fresh: Vec<crate::model::EpisodeDto> = if had_any {
            parsed
                .episodes
                .iter()
                .filter(|e| {
                    e.guid
                        .as_deref()
                        .map(|g| !known.contains(g))
                        .unwrap_or(true)
                })
                .cloned()
                .collect()
        } else {
            parsed.episodes.clone()
        };

        let mut new_count = 0u32;
        if !fresh.is_empty() {
            db::episode::insert_many(conn, &fresh, is_new)?;
            if is_new {
                #[allow(clippy::cast_possible_truncation)]
                {
                    new_count = fresh.len() as u32;
                }
            }
        }

        db::subscription::mark_checked(conn, &parsed.feed_url, etag, last_modified, now)?;
        db::subscription::set_new_count(conn, &parsed.feed_url, i64::from(new_count))?;

        let pruned = db::episode::prune_beyond(conn, &parsed.feed_url, KEEP_EPISODES)?;
        if pruned > 0 {
            tracing::info!(pruned, origin = %parsed.feed_url, "清理过期单集");
        }

        Ok(new_count)
    })
}
