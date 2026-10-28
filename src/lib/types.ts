/** 跨 Rust / TS 边界的数据传输对象，字段顺序与 Rust 侧 struct 一致 */

export interface Episode {
  /** `{feedUrl}:{audioUrl}`，全局唯一，数据库主键 */
  id: string;
  guid: string | null;
  /** 所属播客的 RSS 地址 */
  origin: string;
  title: string;
  podcastTitle: string;
  audioUrl: string;
  imageUrl: string | null;
  /** 秒。RSS 的 itunes:duration 可能是 "1:26:54" 或纯秒数 */
  durationSec: number | null;
  /** epoch 毫秒 */
  publishedMs: number | null;
  description: string | null;
  /** 上次听到哪（秒），没播过是 null */
  positionSec: number | null;
  /** 粘性位：听完过一次就一直是 true */
  finished: boolean;
  /** 所属播客的跳过片头设置（秒） */
  skipIntroSec: number;
  /** 所属播客的跳过片尾设置（秒） */
  skipOutroSec: number;
}

/** 队列项。裸 URL 项没有 episodeId，靠 title/audioUrl 直接播 */
export interface QueueItem {
  id: number;
  episodeId: string | null;
  title: string;
  audioUrl: string | null;
  imageUrl: string | null;
  durationSec: number | null;
  podcastTitle: string | null;
}

export interface Podcast {
  origin: string;
  title: string;
  /** 用户改过名才非空，展示时优先用它 */
  customTitle: string;
  author: string | null;
  description: string | null;
  imageUrl: string | null;
  link: string | null;
  language: string | null;
  skipIntroSec: number;
  skipOutroSec: number;
  createdAt: number;
  episodeCount: number;
  newCount: number;
}

export interface PlayState {
  episodeId: string;
  positionSec: number;
  durationSec: number;
  finished: boolean;
  updatedAt: number;
}

export interface PlayerSession {
  currentEpisode: string | null;
  positionMs: number;
  speed: number;
  volume: number;
}

/** 目录里的一条节目（Apple 搜索 / 榜单结果） */
export interface PodcastPreview {
  id: string;
  title: string;
  author: string | null;
  /** 订阅要用的 RSS 地址 */
  feedUrl: string;
  imageUrl: string | null;
  genre: string | null;
  episodeCount: number;
}

export interface RefreshOutcome {
  origin: string;
  newEpisodes: number;
  notModified: boolean;
  skipped: boolean;
}

export interface RefreshSummary {
  total: number;
  totalNew: number;
  failed: string[];
  elapsedMs: number;
}

export interface OpmlImportReport {
  imported: number;
  skipped: number;
  errors: string[];
}

export interface LegacyCounts {
  podcasts: number;
  episodes: number;
  playStates: number;
  history: number;
  queueItems: number;
}

export interface LegacySkip {
  orphanPlayState: number;
  emptyPlayState: number;
  orphanHistory: number;
  queueWithoutEpisode: number;
  orphanEpisode: number;
}

export interface LegacyScanReport {
  dbPath: string;
  usable: boolean;
  reason: string | null;
  source: LegacyCounts;
  plan: LegacyCounts;
  skipped: LegacySkip;
}

export interface ImportReport {
  plan: LegacyCounts;
  skipped: LegacySkip;
  reportPath: string | null;
  elapsedMs: number;
}
