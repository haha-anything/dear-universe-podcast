//! 跨 IPC 边界的数据结构。
//!
//! 字段一律 camelCase，和前端 `src/lib/types.ts` 保持一一对应。
//! 改这里必须改那里，两边同时改。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeDto {
    /// `{feedUrl}:{audioUrl}`，只在本次会话内做去重/选中判断用
    pub id: String,
    pub title: String,
    pub podcast_title: String,
    pub audio_url: String,
    pub image_url: Option<String>,
    /// 秒（不是毫秒）。毫秒是 M1 数据库里的事，别在这里乘 1000。
    pub duration_sec: Option<i64>,
    pub published_at: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedDto {
    pub feed_url: String,
    pub title: String,
    pub author: Option<String>,
    pub image_url: Option<String>,
    pub description: Option<String>,
    pub episodes: Vec<EpisodeDto>,
}
