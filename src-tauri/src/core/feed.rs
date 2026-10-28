//! RSS 抓取与解析。
//!
//! 这一层的产出 [`FeedDto`] 既是「预览」的数据源，也是订阅入库的原料，
//! 所以 id / guid / pub_date 这些字段必须按数据库的口径生成：
//!   - `id` = `{feedUrl}:{audioUrl}`（§8.4 规定的主键格式）
//!   - 时长保持秒，由 `core/db/episode.rs` 在入库时 ×1000
//!
//! M2 会把这里拆成「无条件预览」与「带 ETag 的条件刷新」两条路径。

use feed_rs::model::{Entry, Feed};
use reqwest::Client;

use crate::{
    error::AppError,
    model::{EpisodeDto, FeedDto},
};

/// 单集列表上限。有些 feed 上千条，M0 阶段没必要全塞给前端。
const MAX_EPISODES: usize = 500;

pub async fn fetch_feed(client: &Client, feed_url: &str) -> Result<FeedDto, AppError> {
    match fetch_conditional(client, feed_url, None, None).await? {
        Fetched::Changed { feed, .. } => Ok(feed),
        // 不带缓存头却回 304 是不可能发生的事，真发生了就当失败
        Fetched::NotModified => Err(AppError::Http { status: 304 }),
    }
}

/// 一次抓取的结果
pub enum Fetched {
    /// 内容有变。顺带把响应里的缓存头带出来，下次请求要带上。
    Changed {
        feed: FeedDto,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// 服务端回 304：内容没变，连解析都省了
    NotModified,
}

/// 带条件头的抓取。刷新走这条：命中 304 就不解析，省流量也省 CPU（§17.4）。
pub async fn fetch_conditional(
    client: &Client,
    feed_url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<Fetched, AppError> {
    tracing::info!(feed_url, has_etag = etag.is_some(), "拉取 RSS");

    let mut req = client.get(feed_url);
    if let Some(tag) = etag.filter(|s| !s.is_empty()) {
        req = req.header(reqwest::header::IF_NONE_MATCH, tag);
    }
    if let Some(since) = last_modified.filter(|s| !s.is_empty()) {
        req = req.header(reqwest::header::IF_MODIFIED_SINCE, since);
    }

    let resp = req.send().await?;
    let status = resp.status();

    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Fetched::NotModified);
    }
    if !status.is_success() {
        return Err(AppError::Http {
            status: status.as_u16(),
        });
    }

    let etag = header_string(resp.headers().get(reqwest::header::ETAG));
    let last_modified = header_string(resp.headers().get(reqwest::header::LAST_MODIFIED));

    let bytes = resp.bytes().await?;
    let feed = feed_rs::parser::parse(&bytes[..]).map_err(|e| AppError::FeedParse(e.to_string()))?;

    tracing::info!(entries = feed.entries.len(), "RSS 解析完成");

    Ok(Fetched::Changed {
        feed: convert(feed_url, &feed),
        etag,
        last_modified,
    })
}

fn header_string(value: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    value
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn convert(feed_url: &str, feed: &Feed) -> FeedDto {
    let title = text_of(feed.title.as_ref()).unwrap_or_else(|| feed_url.to_string());
    let author = feed
        .authors
        .first()
        .map(|p| p.name.clone())
        .filter(|s| !s.trim().is_empty());
    // RSS 的 <image> 和 itunes:image 都会被 feed-rs 归到 logo / icon 这两个坑里
    let cover = pick_feed_image(feed);

    let episodes: Vec<EpisodeDto> = feed
        .entries
        .iter()
        .take(MAX_EPISODES)
        .filter_map(|entry| convert_entry(entry, feed_url, &title, cover.as_deref()))
        .collect();

    FeedDto {
        feed_url: feed_url.to_string(),
        title,
        author,
        image_url: cover,
        description: text_of(feed.description.as_ref()),
        episodes,
    }
}

fn convert_entry(
    entry: &Entry,
    feed_url: &str,
    podcast_title: &str,
    fallback_cover: Option<&str>,
) -> Option<EpisodeDto> {
    let audio_url = pick_audio(entry)?;

    let description = text_of(entry.summary.as_ref()).or_else(|| {
        entry
            .content
            .as_ref()
            .and_then(|c| c.body.as_ref().map(|b| strip_tags(b)))
    });

    Some(EpisodeDto {
        id: format!("{feed_url}:{audio_url}"),
        guid: Some(entry.id.clone()),
        origin: feed_url.to_string(),
        title: text_of(entry.title.as_ref()).unwrap_or_else(|| "(无标题)".to_string()),
        podcast_title: podcast_title.to_string(),
        audio_url: audio_url.clone(),
        image_url: pick_episode_image(entry).or_else(|| fallback_cover.map(str::to_string)),
        duration_sec: pick_duration(entry),
        published_ms: entry.published.map(|dt| dt.timestamp_millis()),
        description,
        position_sec: None,
        finished: false,
        // 跳过片头片尾是播客级设置，解析 feed 时无从得知，由读库时 JOIN 补上
        skip_intro_sec: 0,
        skip_outro_sec: 0,
    })
}

/// 音频地址。脏 feed 的写法很多，按可靠性依次降级：
///   1. media content（RSS 的 <enclosure type="audio/..."> 落在这里）
///   2. 没标类型的 enclosure
///   3. link[type^="audio/"]
fn pick_audio(entry: &Entry) -> Option<String> {
    let mut contents = entry.media.iter().flat_map(|media| media.content.iter());

    // mediatype 是 feed-rs 的传递依赖，不直接声明，所以这里内联调用而不写类型路径
    let audio_typed = contents
        .clone()
        .find(|c| {
            c.content_type
                .as_ref()
                .is_some_and(|mime| mime.as_str().starts_with("audio/"))
        })
        .and_then(content_url);

    let untyped = contents.find(|c| c.url.is_some()).and_then(content_url);

    let from_link = entry.links.iter().find_map(|link| {
        let media_type = link.media_type.as_deref()?;
        if media_type.starts_with("audio/") {
            Some(link.href.clone())
        } else {
            None
        }
    });

    audio_typed.or(untyped).or(from_link)
}

fn content_url(content: &feed_rs::model::MediaContent) -> Option<String> {
    content.url.as_ref().map(|u| u.to_string())
}

/// 时长。`<itunes:duration>` 可能被 feed-rs 塞在 MediaObject 或 MediaContent 上，两边都看
fn pick_duration(entry: &Entry) -> Option<i64> {
    entry
        .media
        .iter()
        .find_map(|media| media.duration.or(content_duration(media)))
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn content_duration(media: &feed_rs::model::MediaObject) -> Option<std::time::Duration> {
    media.content.iter().find_map(|c| c.duration)
}

fn pick_episode_image(entry: &Entry) -> Option<String> {
    entry
        .media
        .iter()
        .flat_map(|media| media.thumbnails.iter())
        .map(|thumb| thumb.image.uri.clone())
        .next()
}

fn pick_feed_image(feed: &Feed) -> Option<String> {
    feed.logo
        .as_ref()
        .map(|img| img.uri.clone())
        .or_else(|| feed.icon.as_ref().map(|img| img.uri.clone()))
}

fn text_of(text: Option<&feed_rs::model::Text>) -> Option<String> {
    text.map(|t| t.content.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// RSS 描述里全是 HTML，前端其实也会剥一遍，这里先粗剥一层减少传输量
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let collapsed: Vec<&str> = out.split_whitespace().collect();
    collapsed.join(" ")
}
