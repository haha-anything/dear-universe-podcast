import { invoke } from "@tauri-apps/api/core";
import type { Podcast } from "../types";

/**
 * 所有 IPC 都从这里走，组件不许直接 invoke（AGENTS.md 代码规范）。
 * Rust 命令返回 Result<T, String>，失败时 invoke 会 reject 并带上错误文本。
 */

/** 订阅：抓一次 feed 并写库。返回落库后的节目信息（带计数）。 */
export function feedSubscribe(url: string): Promise<Podcast> {
  return invoke<Podcast>("feed_subscribe", { url });
}

export function feedList(): Promise<Podcast[]> {
  return invoke<Podcast[]>("feed_list");
}

export function feedUnsubscribe(origin: string): Promise<void> {
  return invoke<void>("feed_unsubscribe", { origin });
}

/** 改名 / 跳过片头 / 跳过片尾。改名只能改 customTitle，title 会被刷新覆盖。 */
export function podcastUpdateSettings(
  origin: string,
  customTitle: string,
  skipIntroSec: number,
  skipOutroSec: number,
): Promise<void> {
  return invoke<void>("podcast_update_settings", {
    origin,
    customTitle,
    skipIntroSec,
    skipOutroSec,
  });
}
