import { progressGet, progressSave, sessionSave } from "./api/player";
import type { Episode } from "./types";

/** 倍速档位，0.5–3.0 之间，符合任务书 §10 */
export const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 1.75, 2, 3] as const;

/** 进度写库的最小间隔。timeupdate 一秒来 4 次，不能每次都打 IPC（§13） */
const FLUSH_INTERVAL_MS = 5_000;

/** 听完整到这个比例就算「听完」，和 Rust 侧 play_state 保持一致 */
const FINISH_RATIO = 0.98;

/**
 * 全局音频播放器状态。
 *
 * 用 runes 而不是 store 包一层（AGENTS.md 禁令第 3 条），
 * 所以文件名必须是 `.svelte.ts` —— 普通 `.ts` 里 `$state` 编译不过。
 *
 * 音频元素自身挂在 App 上，挂载后 attach 进来，本类只管状态和事件。
 * 播放进度的持久化也在这里：节流写 progress_save，换集/暂停时立刻 flush。
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
  #lastFlushAt = 0;

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
      this.#maybeFlush();
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
      void this.#flush();
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
      // 落库：把位置顶到时长，Rust 侧据此把 finished 置上
      void this.#flush();
    });
    on("error", () => {
      this.playing = false;
      this.buffering = false;
      this.error = describeMediaError(el.error);
    });

    el.playbackRate = this.rate;

    return () => {
      void this.#flush();
      for (const off of this.#cleanups) off();
      this.#cleanups = [];
      this.#el = null;
    };
  }

  /**
   * 播放某一集；点的是当前集就只做暂停/恢复。
   * 切集会先把上一集的进度写掉，避免丢进度。
   */
  async toggle(episode: Episode): Promise<void> {
    const el = this.#el;
    if (!el) return;

    if (this.current?.id === episode.id) {
      if (el.paused) void el.play();
      else el.pause();
      return;
    }

    await this.#flush();

    const startSec = await startPositionOf(episode);
    this.current = episode;
    this.positionSec = startSec;
    this.durationSec = episode.durationSec ?? 0;
    this.error = null;
    this.buffering = true;
    this.#lastFlushAt = Date.now();

    el.src = episode.audioUrl;
    el.load();
    this.#armStartAt(startSec);

    void el.play().catch((e: unknown) => {
      this.playing = false;
      this.buffering = false;
      this.error = e instanceof Error ? e.message : String(e);
    });
  }

  /** 重启恢复：把上次的集和位置摆回界面，**不自动播** */
  restore(episode: Episode, positionSec: number): void {
    const el = this.#el;
    if (!el) return;

    this.current = episode;
    this.positionSec = positionSec;
    this.durationSec = episode.durationSec ?? 0;
    el.src = episode.audioUrl;
    el.load();
    this.#armStartAt(positionSec);
  }

  seek(sec: number): void {
    const el = this.#el;
    if (!el || !Number.isFinite(el.duration) || el.duration <= 0) return;
    el.currentTime = Math.min(Math.max(sec, 0), el.duration);
    this.positionSec = el.currentTime;
    this.#maybeFlush();
  }

  skip(deltaSec: number): void {
    this.seek((this.#el?.currentTime ?? 0) + deltaSec);
  }

  setRate(rate: number): void {
    this.rate = rate;
    if (this.#el) this.#el.playbackRate = rate;
    void this.#flush();
  }

  /** 元数据就绪才能设 currentTime，早设会被忽略 */
  #armStartAt(sec: number): void {
    const el = this.#el;
    if (!el || sec <= 0) return;

    const apply = (): void => {
      el.removeEventListener("loadedmetadata", apply);
      try {
        el.currentTime = sec;
      } catch {
        // CDN 不给 Range 时 seek 会抛，忽略：从头播也好过崩掉
      }
    };
    el.addEventListener("loadedmetadata", apply);
    this.#cleanups.push(() => el.removeEventListener("loadedmetadata", apply));
  }

  #maybeFlush(): void {
    if (Date.now() - this.#lastFlushAt < FLUSH_INTERVAL_MS) return;
    void this.#flush();
  }

  async #flush(): Promise<void> {
    this.#lastFlushAt = Date.now();

    const ep = this.current;
    if (!ep) return;

    const position = this.#el?.currentTime ?? this.positionSec;
    const duration = this.durationSec || (this.#el?.duration ?? 0);
    if (!Number.isFinite(position) || position <= 0) return;

    await progressSave(ep.id, position, Number.isFinite(duration) ? duration : 0).catch(
      () => undefined,
    );
    await sessionSave(ep.id, position, this.rate, 100).catch(() => undefined);
  }
}

/**
 * 这一集该从哪开始播。
 * 列表查询已经 LEFT JOIN 过 play_state，通常不用再问一遍；
 * 只有在单集对象里没带出进度时才补一次 IPC。
 */
async function startPositionOf(episode: Episode): Promise<number> {
  let sec = episode.positionSec ?? 0;

  if (sec <= 0) {
    const state = await progressGet(episode.id).catch(() => null);
    sec = state?.positionSec ?? 0;
  }

  if (sec <= 0) return 0;

  // 听完了就从头来（finished 是粘性位，能走到这里说明它是被覆盖过的）
  if (episode.finished) return 0;
  const total = episode.durationSec ?? 0;
  if (total > 0 && sec / total > FINISH_RATIO) return 0;

  return sec;
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
