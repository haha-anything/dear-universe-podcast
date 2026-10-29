//! Apple Podcasts 公开目录（搜索 / 榜单 / lookup）。
//!
//! 合规红线写在 `sources/mod.rs`，改这里之前先读。
//!
//! 两条纪律：
//!   1. **country 默认 cn**。参考实现的 bug 就是默认了 us，中文搜索命中美国区（§6.1）
//!   2. **失败当没结果**，不往上抛。Apple 挂了其余功能必须照常（§6.3）

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;

use crate::{error::AppError, model::PodcastPreviewDto};

const SEARCH_URL: &str = "https://itunes.apple.com/search";
const LOOKUP_URL: &str = "https://itunes.apple.com/lookup";

/// 限流约 20 req/min/IP，超了 403（§6.1）
const TIMEOUT: Duration = Duration::from_secs(15);

/// 缓存时长：搜索 24h / 榜单 6h / lookup 永久
const SEARCH_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const TOP_TTL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

/// Apple 返回的键是 camelCase（`feedUrl` / `collectionName`），
/// serde **不会**自动做大小写转换，必须显式 rename_all。
/// 漏了这一行的表现很阴：status 200、results 有 10 条、但每个字段都是 None。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    #[serde(default)]
    collection_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    feed_url: Option<String>,
    #[serde(default)]
    artwork_url600: Option<String>,
    #[serde(default)]
    collection_id: Option<i64>,
    #[serde(default)]
    primary_genre_name: Option<String>,
    #[serde(default)]
    track_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct TopResponse {
    feed: TopFeed,
}

/// 榜单的字段是 `feed.entry`，不是 `feed.results` —— 跟搜索接口不是一套命名
#[derive(Debug, Clone, Deserialize)]
struct TopFeed {
    #[serde(default)]
    entry: Vec<TopEntry>,
}

/// 榜单条目的值都裹在 `{"label": ...}` 里，这是 iTunes RSS 的老格式
#[derive(Debug, Clone, Deserialize)]
struct TopEntry {
    #[serde(rename = "im:name")]
    name: Label,
    #[serde(rename = "im:artist")]
    artist: Option<Label>,
    id: TopId,
    #[serde(rename = "im:image")]
    image: Option<Vec<Label>>,
}

#[derive(Debug, Clone, Deserialize)]
struct Label {
    label: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TopId {
    attributes: TopIdAttrs,
}

#[derive(Debug, Clone, Deserialize)]
struct TopIdAttrs {
    #[serde(rename = "im:id")]
    im_id: String,
}

/// 目录客户端。带一个进程内缓存，省得连点把限流打爆。
pub struct AppleClient {
    client: Client,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    search: HashMap<String, (Instant, Vec<PodcastPreviewDto>)>,
    top: HashMap<String, (Instant, Vec<PodcastPreviewDto>)>,
    lookup: HashMap<i64, PodcastPreviewDto>,
}

impl AppleClient {
    pub fn new() -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(TIMEOUT)
            // 和 http.rs 同理：非浏览器 UA 会被 CDN 直接 403（实测榜单接口也是）
            .user_agent(super::super::http::USER_AGENT_VALUE)
            .build()?;

        Ok(Self {
            client,
            cache: Mutex::new(Cache::default()),
        })
    }

    /// 关键词搜索。`country` 默认 `cn`。
    ///
    /// Apple 不可达时返回**空列表**而不是 Err —— 发现页空着也不能弹错误框。
    pub async fn search(
        &self,
        query: &str,
        country: &str,
        limit: u32,
    ) -> Result<Vec<PodcastPreviewDto>, AppError> {
        let term = query.trim();
        if term.is_empty() {
            return Ok(Vec::new());
        }

        let key = format!("{country}|{term}|{limit}");
        if let Some(hit) = self.cached(&key, true) {
            return Ok(hit);
        }

        let resp = self
            .client
            .get(SEARCH_URL)
            .query(&[
                ("term", term),
                ("media", "podcast"),
                ("country", country),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await;

        let Ok(resp) = resp else {
            tracing::warn!("Apple 搜索不可达");
            return Ok(Vec::new());
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "Apple 搜索返回非 2xx");
            return Ok(Vec::new());
        }

        let parsed: SearchResponse = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "Apple 搜索响应解析失败");
                return Ok(Vec::new());
            }
        };

        let out: Vec<PodcastPreviewDto> = parsed
            .results
            .into_iter()
            .filter_map(|r| {
                let feed_url = r.feed_url.filter(|s| !s.is_empty())?;
                Some(PodcastPreviewDto {
                    id: r.collection_id.unwrap_or_default().to_string(),
                    title: r.collection_name.unwrap_or_else(|| feed_url.clone()),
                    author: r.artist_name,
                    feed_url,
                    image_url: r.artwork_url600,
                    genre: r.primary_genre_name,
                    episode_count: r.track_count.unwrap_or(0),
                })
            })
            .collect();

        self.remember(&key, true, &out);
        Ok(out)
    }

    /// 榜单。country 取 `cn` / `us`。
    pub async fn top(
        &self,
        country: &str,
        limit: u32,
    ) -> Result<Vec<PodcastPreviewDto>, AppError> {
        let key = format!("{country}|{limit}");
        if let Some(hit) = self.cached(&key, false) {
            return Ok(hit);
        }

        let url = format!("https://itunes.apple.com/{country}/rss/toppodcasts/limit={limit}/json");
        let resp = self.client.get(&url).send().await;

        let Ok(resp) = resp else {
            tracing::warn!("Apple 榜单不可达");
            return Ok(Vec::new());
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "Apple 榜单返回非 2xx");
            return Ok(Vec::new());
        }

        let parsed: TopResponse = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "Apple 榜单响应解析失败");
                return Ok(Vec::new());
            }
        };

        let ids: Vec<i64> = parsed
            .feed
            .entry
            .iter()
            .filter_map(|e| e.id.attributes.im_id.parse().ok())
            .collect();

        // 榜单只给 id，feedUrl 得再 lookup 一次才能订
        let detailed = self.lookup(&ids).await.unwrap_or_default();
        let by_id: HashMap<String, PodcastPreviewDto> =
            detailed.into_iter().map(|p| (p.id.clone(), p)).collect();

        let mut out = Vec::new();
        for entry in &parsed.feed.entry {
            let id = entry.id.attributes.im_id.clone();
            if let Some(found) = by_id.get(&id) {
                // 用榜单自己的封面兜底：lookup 给的 artwork600 有时更糊
                let mut item = found.clone();
                if item.image_url.is_none() {
                    item.image_url = largest_image(entry);
                }
                out.push(item);
            }
            // lookup 没命中的直接跳过：榜单条目没有 feedUrl，
            // 给用户一个订不了的按钮比不给更糟（§6.2 不许转存，也就没得兜底）
            else {
                tracing::debug!(
                    name = %entry.name.label,
                    artist = %entry.artist.as_ref().map(|a| a.label.as_str()).unwrap_or(""),
                    "榜单条目没查到 feedUrl，跳过"
                );
            }
        }

        self.remember(&key, false, &out);
        Ok(out)
    }

    /// 批量取详情。lookup 结果永久缓存（feedUrl 基本不变，§6.1）
    pub async fn lookup(&self, ids: &[i64]) -> Result<Vec<PodcastPreviewDto>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let joined = ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");

        let resp = self
            .client
            .get(LOOKUP_URL)
            .query(&[("id", joined.as_str()), ("entity", "podcast")])
            .send()
            .await;

        let Ok(resp) = resp else {
            return Ok(Vec::new());
        };
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let parsed: SearchResponse = match resp.json().await {
            Ok(v) => v,
            Err(_) => return Ok(Vec::new()),
        };

        let out: Vec<PodcastPreviewDto> = parsed
            .results
            .into_iter()
            .filter_map(|r| {
                let feed_url = r.feed_url.filter(|s| !s.is_empty())?;
                Some(PodcastPreviewDto {
                    id: r.collection_id.unwrap_or_default().to_string(),
                    title: r.collection_name.unwrap_or_else(|| feed_url.clone()),
                    author: r.artist_name,
                    feed_url,
                    image_url: r.artwork_url600,
                    genre: r.primary_genre_name,
                    episode_count: r.track_count.unwrap_or(0),
                })
            })
            .collect();

        // lookup 永久缓存
        if let Ok(mut cache) = self.cache.lock() {
            for item in &out {
                if let Ok(id) = item.id.parse::<i64>() {
                    cache.lookup.insert(id, item.clone());
                }
            }
        }

        Ok(out)
    }

    fn cached(&self, key: &str, is_search: bool) -> Option<Vec<PodcastPreviewDto>> {
        let cache = self.cache.lock().ok()?;
        let entry = if is_search {
            cache.search.get(key)
        } else {
            cache.top.get(key)
        };

        let (at, value) = entry?;
        let ttl = if is_search { SEARCH_TTL } else { TOP_TTL };
        (at.elapsed() < ttl).then(|| value.clone())
    }

    fn remember(&self, key: &str, is_search: bool, value: &[PodcastPreviewDto]) {
        if let Ok(mut cache) = self.cache.lock() {
            let slot = (Instant::now(), value.to_vec());
            if is_search {
                cache.search.insert(key.to_string(), slot);
            } else {
                cache.top.insert(key.to_string(), slot);
            }
        }
    }
}

/// 榜单的 im:image 从大到小排，取第一个（最大）
fn largest_image(entry: &TopEntry) -> Option<String> {
    entry.image.as_ref()?.first().map(|i| i.label.clone())
}
