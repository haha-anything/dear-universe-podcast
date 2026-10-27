/** 跨 Rust / TS 边界的数据传输对象，字段顺序与 Rust 侧 struct 一致 */

export interface Episode {
  id: string;
  title: string;
  podcastTitle: string;
  audioUrl: string;
  imageUrl: string | null;
  /** 秒。RSS 的 itunes:duration 可能是 "1:26:54" 或纯秒数 */
  durationSec: number | null;
  /** RFC3339 字符串，可能为 null */
  publishedAt: string | null;
  description: string | null;
}

export interface FeedPreview {
  feedUrl: string;
  title: string;
  author: string | null;
  imageUrl: string | null;
  description: string | null;
  episodes: Episode[];
}
