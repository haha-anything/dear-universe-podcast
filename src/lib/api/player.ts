import { invoke } from "@tauri-apps/api/core";
import type { PlayState, PlayerSession } from "../types";

/** 存进度。front-end 每 5 秒调一次，暂停/切集时立刻 flush。 */
export function progressSave(
  episodeId: string,
  positionSec: number,
  durationSec: number,
): Promise<void> {
  return invoke<void>("progress_save", { episodeId, positionSec, durationSec });
}

export function progressGet(episodeId: string): Promise<PlayState | null> {
  return invoke<PlayState | null>("progress_get", { episodeId });
}

/** 会话 = 「界面上正在选中哪一集」，真正的进度走 play_state */
export function sessionSave(
  episodeId: string | null,
  positionSec: number,
  speed: number,
  volume: number,
): Promise<void> {
  return invoke<void>("session_save", { episodeId, positionSec, speed, volume });
}

export function sessionLoad(): Promise<PlayerSession | null> {
  return invoke<PlayerSession | null>("session_load");
}
