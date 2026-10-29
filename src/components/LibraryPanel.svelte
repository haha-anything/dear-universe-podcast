<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import {
    downloadCancel,
    downloadList,
    downloadPause,
    downloadResume,
    downloadSetLimit,
    downloadStart,
    favoriteList,
    favoriteRemove,
    historyClear,
    historyList,
    historyRemove,
    progressListRecent,
  } from "../lib/api/library";
  import { formatBytes, formatDuration, formatRelative } from "../lib/format";
  import { player } from "../lib/player.svelte";
  import type { DownloadProgressEvent, DownloadTask, Episode, FavoriteEntry, HistoryEntry } from "../lib/types";

  let tab = $state<"history" | "favorites" | "downloads">("history");

  let history = $state<HistoryEntry[]>([]);
  let favorites = $state<FavoriteEntry[]>([]);
  let downloads = $state<DownloadTask[]>([]);
  let resume = $state<Episode[]>([]);
  let loading = $state(false);
  let note = $state<string | null>(null);

  /** 下载进度：key 是 episodeId，只保留正在跑的 */
  let speeds = $state<Record<string, DownloadProgressEvent>>({});

  const LIMIT_PRESETS: ReadonlyArray<{ label: string; bytes: number }> = [
    { label: "不限速", bytes: 0 },
    { label: "1 MB/s", bytes: 1024 * 1024 },
    { label: "500 KB/s", bytes: 512 * 1024 },
    { label: "128 KB/s", bytes: 128 * 1024 },
  ];

  async function load(): Promise<void> {
    loading = true;
    try {
      const [h, f, d, r] = await Promise.all([
        historyList(200),
        favoriteList(200),
        downloadList(),
        progressListRecent(20),
      ]);
      history = h;
      favorites = f;
      downloads = d;
      resume = r;
    } catch (e) {
      note = describe(e);
    } finally {
      loading = false;
    }
  }

  function progressOf(task: DownloadTask): number {
    if (task.status === "done") return 1;
    const live = speeds[task.episodeId];
    const total = live?.total || task.total;
    if (total <= 0) return 0;
    return (live?.received ?? task.received) / total;
  }

  async function startDownload(episodeId: string): Promise<void> {
    try {
      await downloadStart(episodeId);
      await load();
    } catch (e) {
      note = describe(e);
    }
  }

  async function cancelDownload(episodeId: string): Promise<void> {
    try {
      await downloadCancel(episodeId);
      await load();
    } catch (e) {
      note = describe(e);
    }
  }

  async function dropFavorite(episodeId: string): Promise<void> {
    await favoriteRemove(episodeId).catch((e) => (note = describe(e)));
    await load();
  }

  async function dropHistory(id: number): Promise<void> {
    await historyRemove(id).catch((e) => (note = describe(e)));
    await load();
  }

  async function clearHistory(): Promise<void> {
    if (!window.confirm("清空全部播放历史？")) return;
    await historyClear().catch((e) => (note = describe(e)));
    await load();
  }

  function describe(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return String(e);
  }

  $effect(() => {
    void load();
  });

  // 下载进度事件由 Rust 侧推，窗口藏着也在跑
  $effect(() => {
    const un = listen<DownloadProgressEvent>("download-progress", (e) => {
      speeds = { ...speeds, [e.payload.episodeId]: e.payload };
    });
    const un2 = listen<{ episodeId: string }>("download-done", () => void load());
    const un3 = listen<{ episodeId: string; message: string }>("download-failed", (e) => {
      note = `下载失败：${e.payload.message}`;
      void load();
    });
    return () => {
      void un.then((f) => f());
      void un2.then((f) => f());
      void un3.then((f) => f());
    };
  });
</script>

<section class="flex min-h-0 flex-1 flex-col">
  <header class="flex shrink-0 items-center gap-2 border-b border-line bg-surface px-4 py-3">
    {#each [{ id: "history", label: "历史" }, { id: "favorites", label: "收藏" }, { id: "downloads", label: "下载" }] as const as item (item.id)}
      <button
        type="button"
        onclick={() => (tab = item.id)}
        class="h-8 rounded-pill border border-line px-3 text-xs {tab === item.id
          ? 'bg-canvas text-ink'
          : 'text-ink-sub hover:text-ink'}">{item.label}</button
      >
    {/each}
    <span class="flex-1"></span>
    {#if tab === "history" && history.length > 0}
      <button
        type="button"
        onclick={() => void clearHistory()}
        class="h-8 rounded-pill border border-line px-3 text-xs text-ink-sub hover:text-ink">清空</button
      >
    {/if}
  </header>

  {#if note}
    <p class="shrink-0 border-b border-line bg-canvas px-4 py-2 text-xs text-ink-sub">{note}</p>
  {/if}

  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if loading}
      <p class="p-8 text-center text-sm text-ink-sub">载入中…</p>
    {:else if tab === "history"}
      {#if resume.length > 0}
        <h3 class="px-4 pt-3 text-xs text-ink-sub">继续听</h3>
        <ul>
          {#each resume as ep (ep.id)}
            <li>
              <button
                type="button"
                onclick={() => void player.toggle(ep)}
                class="flex w-full items-center gap-3 border-b border-line px-4 py-2 text-left hover:bg-surface"
              >
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-xs">{ep.title}</span>
                  <span class="block truncate text-[11px] text-ink-sub"
                    >{ep.podcastTitle} · 听到 {formatDuration(ep.positionSec ?? 0)}</span
                  >
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if history.length === 0}
        <p class="p-8 text-center text-sm text-ink-sub">还没有播放记录</p>
      {:else}
        <ul>
          {#each history as entry (entry.id)}
            <li class="group flex items-center gap-3 border-b border-line px-4 py-2">
              <button
                type="button"
                onclick={() => void player.toggle(entry.episode)}
                class="flex min-w-0 flex-1 items-center gap-3 text-left"
              >
                {#if entry.episode.imageUrl}
                  <img
                    src={entry.episode.imageUrl}
                    alt=""
                    loading="lazy"
                    class="h-9 w-9 shrink-0 rounded-cover border border-line object-cover"
                  />
                {:else}
                  <div class="h-9 w-9 shrink-0 rounded-cover border border-line bg-canvas"></div>
                {/if}
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-xs">{entry.episode.title}</span>
                  <span class="block truncate text-[11px] text-ink-sub"
                    >{entry.episode.podcastTitle} · {formatRelative(entry.playedAt)}</span
                  >
                </span>
              </button>
              <button
                type="button"
                onclick={() => void dropHistory(entry.id)}
                class="h-6 w-6 shrink-0 rounded-pill text-xs text-ink-weak hover:text-accent"
                aria-label="删除这条记录">×</button
              >
            </li>
          {/each}
        </ul>
      {/if}
    {:else if tab === "favorites"}
      {#if favorites.length === 0}
        <p class="p-8 text-center text-sm text-ink-sub">还没有收藏。在单集上点 ☆ 加进来。</p>
      {:else}
        <ul>
          {#each favorites as entry (entry.episode.id)}
            <li class="flex items-center gap-3 border-b border-line px-4 py-2">
              <button
                type="button"
                onclick={() => void player.toggle(entry.episode)}
                class="flex min-w-0 flex-1 items-center gap-3 text-left"
              >
                {#if entry.episode.imageUrl}
                  <img
                    src={entry.episode.imageUrl}
                    alt=""
                    loading="lazy"
                    class="h-9 w-9 shrink-0 rounded-cover border border-line object-cover"
                  />
                {:else}
                  <div class="h-9 w-9 shrink-0 rounded-cover border border-line bg-canvas"></div>
                {/if}
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-xs">{entry.episode.title}</span>
                  <span class="block truncate text-[11px] text-ink-sub"
                    >{entry.episode.podcastTitle} · {formatDuration(entry.episode.durationSec)}</span
                  >
                </span>
              </button>
              <button
                type="button"
                onclick={() => void startDownload(entry.episode.id)}
                class="h-7 shrink-0 rounded-pill border border-line px-2 text-[11px] text-ink-sub hover:text-ink"
                >下载</button
              >
              <button
                type="button"
                onclick={() => void dropFavorite(entry.episode.id)}
                class="h-6 w-6 shrink-0 rounded-pill text-xs text-ink-weak hover:text-accent"
                aria-label="取消收藏">×</button
              >
            </li>
          {/each}
        </ul>
      {/if}
    {:else}
      <div class="flex items-center gap-2 border-b border-line px-4 py-2">
        <span class="text-xs text-ink-sub">限速</span>
        {#each LIMIT_PRESETS as preset (preset.bytes)}
          <button
            type="button"
            onclick={() => void downloadSetLimit(preset.bytes)}
            class="h-7 rounded-pill border border-line px-2 text-[11px] text-ink-sub hover:text-ink"
            >{preset.label}</button
          >
        {/each}
      </div>

      {#if downloads.length === 0}
        <p class="p-8 text-center text-sm text-ink-sub">
          还没有下载。在收藏或单集列表里点「下载」，下完可以离线听。
        </p>
      {:else}
        <ul>
          {#each downloads as task (task.episodeId)}
            <li class="border-b border-line px-4 py-2">
              <div class="flex items-center gap-3">
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-xs">{task.title}</span>
                  <span class="block truncate text-[11px] text-ink-sub">
                    {#if task.status === "done"}
                      已完成 · {formatBytes(task.received)}
                    {:else if task.status === "failed"}
                      <span class="text-accent">失败：{task.error ?? "未知原因"}</span>
                    {:else if task.status === "paused"}
                      已暂停 · {formatBytes(task.received)} / {formatBytes(task.total)}
                    {:else}
                      {formatBytes(task.received)} / {formatBytes(task.total)} · {formatBytes(
                        speeds[task.episodeId]?.speed ?? 0,
                      )}/s
                    {/if}
                  </span>
                </span>

                {#if task.status === "downloading"}
                  <button
                    type="button"
                    onclick={() => void downloadPause(task.episodeId)}
                    class="h-7 shrink-0 rounded-pill border border-line px-2 text-[11px] text-ink-sub hover:text-ink"
                    >暂停</button
                  >
                {:else if task.status === "paused"}
                  <button
                    type="button"
                    onclick={() => void downloadResume(task.episodeId)}
                    class="h-7 shrink-0 rounded-pill border border-line px-2 text-[11px] text-ink-sub hover:text-ink"
                    >继续</button
                  >
                {/if}
                <button
                  type="button"
                  onclick={() => void cancelDownload(task.episodeId)}
                  class="h-7 shrink-0 rounded-pill border border-line px-2 text-[11px] text-ink-weak hover:text-accent"
                  >删除</button
                >
              </div>
              <div class="mt-1.5 h-1 w-full rounded-pill bg-track">
                <div
                  class="h-1 rounded-pill bg-accent"
                  style="width: {(progressOf(task) * 100).toFixed(2)}%"
                ></div>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
</section>
