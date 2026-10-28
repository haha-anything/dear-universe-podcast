//! 订阅：抓一次 feed → 节目和单集写进库。
//!
//! 刷新（带 ETag 的条件请求、并发控制）是 M3 的事，那时候在这里加 `refresh_*`。

use reqwest::Client;

use crate::{
    core::db::{self, Database},
    error::AppError,
    model::{FeedDto, PodcastDto},
};

/// 订阅一个 RSS 地址，返回落库后的节目信息。
pub async fn subscribe(
    db: &Database,
    client: &Client,
    feed_url: &str,
) -> Result<PodcastDto, AppError> {
    let parsed = crate::core::feed::fetch_feed(client, feed_url).await?;
    persist(db, &parsed, "rss")
}

/// 把一次抓取的结果写库。
///
/// `is_new` 传 false：首次抓取当基线，不冒出几百条未读（任务书 §13）。
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
