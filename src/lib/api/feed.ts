import { invoke } from "@tauri-apps/api/core";
import type { FeedPreview } from "../types";

/**
 * 所有 IPC 都从这里走，组件不许直接 invoke（AGENTS.md 代码规范）。
 * Rust 命令返回 Result<T, String>，失败时 invoke 会 reject 并带上错误文本。
 */
export function feedPreview(url: string): Promise<FeedPreview> {
  return invoke<FeedPreview>("feed_preview", { url });
}
