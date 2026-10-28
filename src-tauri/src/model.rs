//! 跨 IPC 边界的数据结构。
//!
//! 字段一律 camelCase，和前端 `src/lib/types.ts` 保持一一对应。
//! 改这里必须改那里，两边同时改。
//!
//! 单位约定：DTO 里的时间戳是 **epoch 毫秒**（`publishedMs` / `positionMs`），
//! 时长对外是 **秒**（`durationSec` / `positionSec`）—— 前端只跟秒打交道，
//! 毫秒换算在 Rust 侧做完。唯一的例外是数据库字段 `duration_ms`，那里必须是毫秒。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeDto {
    /// `{feedUrl}:{audioUrl}`，全局唯一，也是数据库主键（任务书 §8.4）
    pub id: String,
    pub guid: Option<String>,
    /// 所属播客的 RSS 地址
    pub origin: String,
    pub title: String,
    pub podcast_title: String,
    pub audio_url: String,
    pub image_url: Option<String>,
    pub duration_sec: Option<i64>,
    /// epoch 毫秒。RSS 给不出发布时间时为空。
    pub published_ms: Option<i64>,
    pub description: Option<String>,
    /// 上次听到哪（秒）。只有播过的才有值。
    pub position_sec: Option<i64>,
    /// 粘性位：听完过一次就永远是 true（§8.7）
    pub finished: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastDto {
    /// RSS 地址，天然去重，也是主键
    pub origin: String,
    pub title: String,
    /// 用户改名后才非空，展示时优先取它
    pub custom_title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub link: Option<String>,
    pub language: Option<String>,
    pub skip_intro_sec: i32,
    pub skip_outro_sec: i32,
    pub created_at: i64,
    /// 库里的单集总数
    pub episode_count: i64,
    /// 未听的单集数（刷新时才会有非零值，见 §13「首次抓取」）
    pub new_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayStateDto {
    pub episode_id: String,
    pub position_sec: i64,
    pub duration_sec: i64,
    pub finished: bool,
    pub updated_at: i64,
}

/// 播放会话，重启后接着播用。单行表，所以这里是「有或没有」。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub current_episode: Option<String>,
    pub position_ms: i64,
    pub speed: f64,
    pub volume: i32,
}

/// dry-run 报告：让用户先看清会写进去什么，再决定要不要导。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyScanReport {
    pub db_path: String,
    /// 文件不存在 / 不是 Podara 库时给 false，附带 readable 的原因
    pub usable: bool,
    pub reason: Option<String>,
    /// 老库里实际躺着的原始行数
    pub source: LegacyCounts,
    /// 真正会被写入的行数
    pub plan: LegacyCounts,
    pub skipped: LegacySkip,
}

/// 导入结果。数字是**实际写入**的行数，和 scan 的计划可能不同（见 skip 字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub plan: LegacyCounts,
    pub skipped: LegacySkip,
    /// import-report.json 的落盘位置（ §15.5）
    pub report_path: Option<String>,
    pub elapsed_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCounts {
    pub podcasts: usize,
    pub episodes: usize,
    pub play_states: usize,
    pub history: usize,
    pub queue_items: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacySkip {
    /// 单集已经不在库里的进度（参考实现攒了 1065 条，见 §15）
    pub orphan_play_state: usize,
    /// 位置为 0 的进度，没迁的意义（5488 条）
    pub empty_play_state: usize,
    /// 单集不在库里的历史
    pub orphan_history: usize,
    /// 找不到对应单集、退化成裸 URL 的队列项
    pub queue_without_episode: usize,
    /// 所属播客没导进来的单集
    pub orphan_episode: usize,
}
