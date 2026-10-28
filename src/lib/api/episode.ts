import { invoke } from "@tauri-apps/api/core";
import type { Episode } from "../types";

/** 某个节目的单集，按发布时间倒序 */
export function episodeList(origin: string, limit = 300, offset = 0): Promise<Episode[]> {
  return invoke<Episode[]>("episode_list", { origin, limit, offset });
}

export function episodeGet(id: string): Promise<Episode | null> {
  return invoke<Episode | null>("episode_get", { id });
}
