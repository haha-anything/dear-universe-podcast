import type { Episode } from "./types";

/** 倍速档位，0.5–3.0 之间，符合任务书 §10 */
export const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 1.75, 2, 3] as const;

/**
 * 全局音频播放器状态。
 *
 * 用 runes 而不是 store 包一层（AGENTS.md 禁令第 3 条），
 * 所以文件名必须是 `.svelte.ts` —— 普通 `.ts` 里 `$state` 编译不过。
 *
 * 音频元素自身挂在 App 上，挂载后 attach 进来，本类只管状态和事件。
 */
class Player {
  current: Episode | null = $state(null);
  playing = $state(false);
  buffering = $state(false);
  positionSec = $state(0);
  durationSec = $state(0);
  rate = $state<number>(1);
  error = $state<string | null>(null);

  #el: HTMLAudioElement | null = null;
  #cleanups: Array<() => void> = [];

  attach(el: HTMLAudioElement): () => void {
    this.#el = el;

    const on = <K extends keyof HTMLMediaElementEventMap>(
      type: K,
      handler: () => void,
    ): void => {
      el.addEventListener(type, handler);
      this.#cleanups.push(() => el.removeEventListener(type, handler));
    };

    on("timeupdate", () => {
      this.positionSec = el.currentTime;
    });
    on("durationchange", () => {
      if (Number.isFinite(el.duration) && el.duration > 0) {
        this.durationSec = el.duration;
      }
    });
    on("play", () => {
      this.playing = true;
    });
    on("pause", () => {
      this.playing = false;
    });
    on("waiting", () => {
      this.buffering = true;
    });
    on("playing", () => {
      this.buffering = false;
      this.error = null;
    });
    on("ended", () => {
      this.playing = false;
      this.positionSec = this.durationSec;
    });
    on("error", () => {
      this.playing = false;
      this.buffering = false;
      this.error = describeMediaError(el.error);
    });

    el.playbackRate = this.rate;

    return () => {
      for (const off of this.#cleanups) off();
      this.#cleanups = [];
      this.#el = null;
    };
  }

  /** 播放某一集；点的是当前集就只做暂停/恢复 */
  toggle(episode: Episode): void {
    const el = this.#el;
    if (!el) return;

    if (this.current?.id === episode.id) {
      if (el.paused) void el.play();
      else el.pause();
      return;
    }

    this.current = episode;
    this.positionSec = 0;
    this.durationSec = 0;
    this.error = null;
    this.buffering = true;
    el.src = episode.audioUrl;
    el.load();
    void el.play().catch((e: unknown) => {
      this.playing = false;
      this.buffering = false;
      this.error = e instanceof Error ? e.message : String(e);
    });
  }

  seek(sec: number): void {
    const el = this.#el;
    if (!el || !Number.isFinite(el.duration) || el.duration <= 0) return;
    el.currentTime = Math.min(Math.max(sec, 0), el.duration);
    this.positionSec = el.currentTime;
  }

  skip(deltaSec: number): void {
    this.seek((this.#el?.currentTime ?? 0) + deltaSec);
  }

  setRate(rate: number): void {
    this.rate = rate;
    if (this.#el) this.#el.playbackRate = rate;
  }
}

function describeMediaError(err: MediaError | null): string {
  if (!err) return "音频加载失败";
  switch (err.code) {
    case MediaError.MEDIA_ERR_ABORTED:
      return "播放被中断";
    case MediaError.MEDIA_ERR_NETWORK:
      return "网络错误，音频下载失败";
    case MediaError.MEDIA_ERR_DECODE:
      return "浏览器无法解码这个音频格式";
    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
      return "音频地址不可用或格式不支持";
    default:
      return "音频加载失败";
  }
}

export const player = new Player();
