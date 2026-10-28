import { invoke } from "@tauri-apps/api/core";
import type { QueueItem } from "../types";

export function queueList(): Promise<QueueItem[]> {
  return invoke<QueueItem[]>("queue_list");
}

/** 加入队尾。已在队列里会静默跳过，不会堆重复项。 */
export function queueAdd(episodeId: string): Promise<void> {
  return invoke<void>("queue_add", { episodeId });
}

export function queueRemove(id: number): Promise<void> {
  return invoke<void>("queue_remove", { id });
}

/** 播完自动出队：手里只有 episodeId */
export function queueRemoveEpisode(episodeId: string): Promise<void> {
  return invoke<void>("queue_remove_episode", { episodeId });
}

/** 拖动排序，传进去的 id 序列就是新顺序 */
export function queueReorder(ids: number[]): Promise<void> {
  return invoke<void>("queue_reorder", { ids });
}

export function queueClear(): Promise<void> {
  return invoke<void>("queue_clear");
}

/** 当前这集播完接哪一集；没有下一集返回 null */
export function queueNext(currentEpisode: string): Promise<QueueItem | null> {
  return invoke<QueueItem | null>("queue_next", { currentEpisode });
}
