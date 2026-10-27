//! RSS 抓取与解析。
//!
//! M0 只做「预览」：给一个 feed 地址，返回节目信息 + 单集列表，不写库。
//! M1 会把这里拆成「持久化 + ETag 条件请求」两条路径。

use feed_rs::model::{Entry, Feed};
use reqwest::Client;

use crate::{
    error::AppError,
    model::{EpisodeDto, FeedDto},
};

/// 单集列表上限。有些 feed 上千条，M0 阶段没必要全塞给前端。
const MAX_EPISODES: usize = 500;

pub async fn fetch_feed(client: &Client, feed_url: &str) -> Result<FeedDto, AppError> {
    tracing::info!(feed_url, "拉取 RSS");

    let resp = client.get(feed_url).send().await?;

    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::Http {
            status: status.as_u16(),
        });
    }

    let bytes = resp.bytes().await?;

    // feed-rs 对脏 XML 容忍度有限（任务书 §14.5）。
    // M0 阶段解析失败就如实报错；兜底的正则降级会在 M1 加上，那里才会影响可用性。
    let feed = feed_rs::parser::parse(&bytes[..]).map_err(|e| AppError::FeedParse(e.to_string()))?;

    tracing::info!(
        entries = feed.entries.len(),
        "RSS 解析完成"
    );

    Ok(convert(feed_url, &feed))
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
        .filter_map(|entry| convert_entry(entry, &title, cover.as_deref()))
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

fn convert_entry(entry: &Entry, podcast_title: &str, fallback_cover: Option<&str>) -> Option<EpisodeDto> {
    let audio_url = pick_audio(entry)?;

    let description = text_of(entry.summary.as_ref()).or_else(|| {
        entry
            .content
            .as_ref()
            .and_then(|c| c.body.as_ref().map(|b| strip_tags(b)))
    });

    Some(EpisodeDto {
        id: format!("{}|{}", podcast_title, audio_url),
        title: text_of(entry.title.as_ref()).unwrap_or_else(|| "(无标题)".to_string()),
        podcast_title: podcast_title.to_string(),
        audio_url: audio_url.clone(),
        image_url: pick_episode_image(entry).or_else(|| fallback_cover.map(str::to_string)),
        duration_sec: pick_duration(entry),
        published_at: entry.published.map(|dt| dt.to_rfc3339()),
        description,
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
