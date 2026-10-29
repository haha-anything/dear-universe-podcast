import { invoke } from "@tauri-apps/api/core";
import type { DownloadTask, Episode, FavoriteEntry, HistoryEntry } from "../types";

export function favoriteList(limit?: number): Promise<FavoriteEntry[]> {
  return invoke<FavoriteEntry[]>("favorite_list", { limit });
}

export function favoriteAdd(episodeId: string): Promise<void> {
  return invoke<void>("favorite_add", { episodeId });
}

export function favoriteRemove(episodeId: string): Promise<void> {
  return invoke<void>("favorite_remove", { episodeId });
}

export function favoriteIds(): Promise<string[]> {
  return invoke<string[]>("favorite_ids");
}

export function historyList(limit?: number): Promise<HistoryEntry[]> {
  return invoke<HistoryEntry[]>("history_list", { limit });
}

export function historyRecord(episodeId: string): Promise<void> {
  return invoke<void>("history_record", { episodeId });
}

export function historyRemove(id: number): Promise<void> {
  return invoke<void>("history_remove", { id });
}

export function historyClear(): Promise<void> {
  return invoke<void>("history_clear");
}

/** 听过但没听完的，按最近听过的排 */
export function progressListRecent(limit?: number): Promise<Episode[]> {
  return invoke<Episode[]>("progress_list_recent", { limit });
}

export function downloadList(): Promise<DownloadTask[]> {
  return invoke<DownloadTask[]>("download_list");
}

export function downloadStart(episodeId: string): Promise<void> {
  return invoke<void>("download_start", { episodeId });
}

export function downloadPause(episodeId: string): Promise<void> {
  return invoke<void>("download_pause", { episodeId });
}

export function downloadResume(episodeId: string): Promise<void> {
  return invoke<void>("download_resume", { episodeId });
}

export function downloadCancel(episodeId: string): Promise<void> {
  return invoke<void>("download_cancel", { episodeId });
}

/** 已下完的本地路径，没下完返回 null */
export function downloadPath(episodeId: string): Promise<string | null> {
  return invoke<string | null>("download_path", { episodeId });
}

/** 限速，字节/秒；0 = 不限 */
export function downloadSetLimit(bytesPerSec: number): Promise<number> {
  return invoke<number>("download_set_limit", { bytesPerSec });
}
