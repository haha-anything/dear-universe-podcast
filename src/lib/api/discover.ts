import { invoke } from "@tauri-apps/api/core";
import type { PodcastPreview, RefreshOutcome, RefreshSummary } from "../types";

/** 榜单。country 只接受 cn / us，后端会兜底成 cn。 */
export function discoverTop(country: string, limit: number): Promise<PodcastPreview[]> {
  return invoke<PodcastPreview[]>("discover_top", { country, limit });
}

export function discoverSearch(
  query: string,
  country: string,
  limit: number,
): Promise<PodcastPreview[]> {
  return invoke<PodcastPreview[]>("discover_search", { query, country, limit });
}

export function feedRefresh(origin: string, force: boolean): Promise<RefreshOutcome> {
  return invoke<RefreshOutcome>("feed_refresh", { origin, force });
}

export function feedRefreshAll(force: boolean): Promise<RefreshSummary> {
  return invoke<RefreshSummary>("feed_refresh_all", { force });
}

export function feedMarkAllPlayed(origin: string): Promise<void> {
  return invoke<void>("feed_mark_all_played", { origin });
}
