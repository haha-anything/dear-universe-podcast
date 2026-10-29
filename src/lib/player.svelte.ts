import { episodeGet } from "./api/episode";
import { downloadPath, historyRecord } from "./api/library";
import { progressGet, progressSave, sessionSave } from "./api/player";
import { queueList, queueNext, queueRemoveEpisode } from "./api/queue";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Episode, QueueItem } from "./types";

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

  /** 播放队列。手动添加，不自动塞整档节目（§17.7） */
  queue: QueueItem[] = $state([]);
  /** 是否播完自动接下一集 */
  autoAdvance = $state(true);

  #el: HTMLAudioElement | null = null;
  #cleanups: Array<() => void> = [];
  #lastFlushAt = 0;
  #skipOutroSec = 0;
  #advancing = false;

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
      this.#maybeAutoAdvance();
    });
    on("durationchange", () => {
      if (Number.isFinite(el.duration) && el.duration > 0) {
        this.durationSec = el.duration;
      }
    });
    on("play", () => {
      this.playing = true;
      this.#syncMediaSession();
      // 每次真正开播记一笔历史；Rust 侧 60s 去重，pause/resume 不会刷屏
      if (this.current) void historyRecord(this.current.id).catch(() => undefined);
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
      void this.#flush().then(() => this.#advance());
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

  // ── 队列 ────────────────────────────────────────────────────────

  async refreshQueue(): Promise<void> {
    this.queue = await queueList().catch(() => []);
  }

  /** 从队列里挑一集播（点队列项） */
  async playQueueItem(item: QueueItem): Promise<void> {
    if (!item.audioUrl) return;

    // 裸 URL 项没有 episode 记录，拼一个临时 Episode 出来播
    const episode: Episode =
      item.episodeId !== null
        ? (await episodeOf(item.episodeId)) ?? synthEpisode(item)
        : synthEpisode(item);

    await this.play(episode);
  }

  /** 播完一集：出队 + 接下一集 */
  async #advance(): Promise<void> {
    // 片尾跳过是靠 timeupdate 触发的，一秒来四次；没有这个闸门会连着出队好几集
    if (this.#advancing) return;
    this.#advancing = true;

    try {
      const finished = this.current;
      if (!finished) return;

      await queueRemoveEpisode(finished.id).catch(() => undefined);
      await this.refreshQueue();

      if (!this.autoAdvance) return;

      const next = await queueNext(finished.id).catch(() => null);
      if (!next) return;

      await this.playQueueItem(next);
    } finally {
      this.#advancing = false;
    }
  }

  // ── 播放控制 ────────────────────────────────────────────────────

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

    await this.play(episode);
  }

  async play(episode: Episode): Promise<void> {
    const el = this.#el;
    if (!el) return;

    await this.#flush();

    const startSec = await startPositionOf(episode);
    // 已下载完成的优先走本地文件，离线也能听，也省流量（§13 下载闭环）
    const src = await this.#resolveSrc(episode);
    this.current = episode;
    this.positionSec = startSec;
    this.durationSec = episode.durationSec ?? 0;
    this.error = null;
    this.buffering = true;
    this.#lastFlushAt = Date.now();
    this.#skipOutroSec = episode.skipOutroSec ?? 0;

    el.src = src;
    el.load();
    this.#armStartAt(startSec);
    this.#syncMediaSession();

    void el.play().catch((e: unknown) => {
      this.playing = false;
      this.buffering = false;
      this.error = e instanceof Error ? e.message : String(e);
    });
  }

  /** 播放 / 暂停切换（托盘菜单、media-key 事件用） */
  togglePlayPause(): void {
    const el = this.#el;
    if (!el || !this.current) return;
    if (el.paused) void el.play();
    else el.pause();
  }

  /** 下一集：优先队列下一项，没有就停 */
  async playNext(): Promise<void> {
    const el = this.#el;
    if (!el) return;
    await this.#flush();
    const next = await queueNext(this.current?.id ?? "").catch(() => null);
    if (next) {
      await this.playQueueItem(next);
    } else {
      el.pause();
    }
  }

  /** 上一集：播过头 3 秒就回到开头，符合常见播放器语义 */
  playPrev(): void {
    const el = this.#el;
    if (!el || !this.current) return;
    if (el.currentTime > 3) void this.seek(0);
  }

  /** 重启恢复：把上次的集和位置摆回界面，**不自动播** */
  restore(episode: Episode, positionSec: number): void {
    const el = this.#el;
    if (!el) return;

    this.current = episode;
    this.positionSec = positionSec;
    this.durationSec = episode.durationSec ?? 0;
    this.#skipOutroSec = episode.skipOutroSec ?? 0;
    el.src = episode.audioUrl; // 同步兜底，保证重启后立刻能点播
    el.load();
    this.#armStartAt(positionSec);
    this.#syncMediaSession();
    // 若这集已下载，异步换成本地文件；用户还没点播时才替换，避免打断
    void this.#preferLocal(el, episode, positionSec);
  }

  /** 已下载完成的返回 asset:// 本地地址，否则返回原始音频 URL */
  async #resolveSrc(episode: Episode): Promise<string> {
    const local = await downloadPath(episode.id).catch(() => null);
    return local ? convertFileSrc(local) : episode.audioUrl;
  }

  /** 恢复播放态后把源换成本地文件；只在用户还没实际点播时替换，避免打断 */
  async #preferLocal(el: HTMLAudioElement, episode: Episode, startSec: number): Promise<void> {
    const local = await downloadPath(episode.id).catch(() => null);
    if (!local) return;
    if (this.current?.id !== episode.id) return; // 已经切到别的集
    if (!el.paused || el.currentTime > 0.5) return; // 用户已开始播，别打断
    el.src = convertFileSrc(local);
    el.load();
    this.#armStartAt(startSec);
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

  /** 片尾跳过：剩这么多秒时就算播完，直接接下一集（§8.8） */
  #maybeAutoAdvance(): void {
    const el = this.#el;
    if (!el || this.#skipOutroSec <= 0 || !this.autoAdvance) return;
    if (!Number.isFinite(el.duration) || el.duration <= 0) return;

    if (el.duration - el.currentTime <= this.#skipOutroSec) {
      // 直接掉 flush+advance，ended 事件不会来了
      void this.#flush().then(() => this.#advance());
    }
  }

  /**
   * 媒体会话（§10）。Chromium 在有真实播放 + metadata 时会把它注册进 Windows SMTC，
   * 硬件媒体键因此可能免费可用 —— M2 先接上，能不能用实测见 docs/M2-验收.md。
   */
  #syncMediaSession(): void {
    if (!("mediaSession" in navigator)) return;

    const ep = this.current;
    if (!ep) return;

    try {
      navigator.mediaSession.metadata = new MediaMetadata({
        title: ep.title,
        artist: ep.podcastTitle,
        artwork: ep.imageUrl ? [{ src: ep.imageUrl, sizes: "512x512", type: "image/jpeg" }] : [],
      });

      navigator.mediaSession.setActionHandler("play", () => void this.#el?.play());
      navigator.mediaSession.setActionHandler("pause", () => this.#el?.pause());
      navigator.mediaSession.setActionHandler("seekbackward", () => this.skip(-15));
      navigator.mediaSession.setActionHandler("seekforward", () => this.skip(30));
      navigator.mediaSession.setActionHandler("previoustrack", () => this.skip(-30));
      navigator.mediaSession.setActionHandler("nexttrack", () => void this.#advance());
      navigator.mediaSession.setActionHandler("stop", () => this.#el?.pause());
      navigator.mediaSession.playbackState = this.playing ? "playing" : "paused";
    } catch {
      // 某些 WebView2 版本没有 MediaMetadata，忽略：媒体键是加分项不是主线
    }
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
 *
 * 跳过片头在这里生效：没听过的情况下直接跳到 skipIntroSec 之后。
 */
async function startPositionOf(episode: Episode): Promise<number> {
  let sec = episode.positionSec ?? 0;

  if (sec <= 0) {
    const state = await progressGet(episode.id).catch(() => null);
    sec = state?.positionSec ?? 0;
  }

  // 没听过的，跳过片头
  if (sec <= 0) return Math.max(episode.skipIntroSec ?? 0, 0);

  // 听完了就从头来（finished 是粘性位，能走到这里说明它是被覆盖过的）
  if (episode.finished) return 0;
  const total = episode.durationSec ?? 0;
  if (total > 0 && sec / total > FINISH_RATIO) return 0;

  return sec;
}

async function episodeOf(id: string): Promise<Episode | null> {
  return episodeGet(id).catch(() => null);
}

/** 裸 URL 队列项没有 episode 记录，现场拼一个够播的对象 */
function synthEpisode(item: QueueItem): Episode {
  return {
    id: `raw:${item.audioUrl ?? item.id}`,
    guid: null,
    origin: "",
    title: item.title,
    podcastTitle: item.podcastTitle ?? "队列",
    audioUrl: item.audioUrl ?? "",
    imageUrl: item.imageUrl,
    durationSec: item.durationSec,
    publishedMs: null,
    description: null,
    positionSec: null,
    finished: false,
    skipIntroSec: 0,
    skipOutroSec: 0,
  };
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
