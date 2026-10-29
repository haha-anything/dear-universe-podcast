<script lang="ts">
  import DiscoverPanel from "./components/DiscoverPanel.svelte";
  import EpisodeList from "./components/EpisodeList.svelte";
  import FeedBar from "./components/FeedBar.svelte";
  import ImportPanel from "./components/ImportPanel.svelte";
  import LibraryPanel from "./components/LibraryPanel.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import PlayerPage from "./components/PlayerPage.svelte";
  import QueueDrawer from "./components/QueueDrawer.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import { feedRefreshAll } from "./lib/api/discover";
  import { episodeGet, episodeList } from "./lib/api/episode";
  import { feedList, feedSubscribe, feedUnsubscribe, podcastUpdateSettings } from "./lib/api/feed";
  import { sessionLoad } from "./lib/api/player";
  import { queueAdd } from "./lib/api/queue";
  import { downloadStart, favoriteAdd, favoriteIds, favoriteRemove } from "./lib/api/library";
  import { stripHtml } from "./lib/format";
  import { listen } from "@tauri-apps/api/event";
  import { player } from "./lib/player.svelte";
  import type { Episode, Podcast } from "./lib/types";

  let podcasts = $state<Podcast[]>([]);
  let selected = $state<string | null>(null);
  let episodes = $state<Episode[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let showImport = $state(false);
  let showPlayerPage = $state(false);
  let showQueue = $state(false);
  let showSettings = $state(false);
  let view = $state<"subscriptions" | "discover" | "library">("subscriptions");
  let refreshing = $state(false);
  let refreshDone = $state(0);
  let refreshTotal = $state(0);
  let favorites = $state<Set<string>>(new Set());
  let audioEl = $state<HTMLAudioElement | null>(null);

  // 频道设置表单（改名 / 跳过片头片尾）
  let editTitle = $state("");
  let editIntro = $state(0);
  let editOutro = $state(0);

  // 快速连点订阅列表时，晚到的旧结果不能覆盖新的
  let loadToken = 0;

  const current = $derived(podcasts.find((p) => p.origin === selected) ?? null);

  // 音频元素挂进来之后交给 player 管；返回的解绑函数由 $effect 负责调用
  $effect(() => {
    if (!audioEl) return;
    return player.attach(audioEl);
  });

  // 刷新进度由 Rust 侧 emit，窗口藏着的时候也在跑，所以挂全局监听
  $effect(() => {
    const un = listen<{ done: number; total: number }>("refresh-progress", (e) => {
      refreshDone = e.payload.done;
      refreshTotal = e.payload.total;
    });
    return () => void un.then((f) => f());
  });

  // 明暗双主题跟随系统（§11.3 要求 V1 就做双主题）
  $effect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme = mq.matches ? "dark" : "light";
    };
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  });

  // 启动时：读订阅列表 + 恢复上次在听的那一集（不自动播）
  $effect(() => {
    void boot();
  });

  async function boot(): Promise<void> {
    const list = await feedList().catch((e) => {
      error = describe(e);
      return [] as Podcast[];
    });
    podcasts = list;
    await loadFavorites();

    if (podcasts[0]) await select(podcasts[0].origin);
    await player.refreshQueue();
    await restoreLastSession();
  }

  async function select(origin: string): Promise<void> {
    const token = ++loadToken;
    selected = origin;
    showSettings = false;

    const list = await episodeList(origin).catch((e) => {
      error = describe(e);
      return [] as Episode[];
    });
    if (token !== loadToken) return; // 已经切到别的节目了，这次结果作废

    episodes = list;
  }

  // 重启接着播：只把状态摆回界面，播放要用户自己点
  async function restoreLastSession(): Promise<void> {
    const session = await sessionLoad().catch(() => null);
    if (!session?.currentEpisode) return;

    const episode = await episodeGet(session.currentEpisode).catch(() => null);
    if (episode) player.restore(episode, session.positionMs / 1000);
  }

  async function reload(): Promise<void> {
    podcasts = await feedList().catch((e) => {
      error = describe(e);
      return [] as Podcast[];
    });
  }

  async function subscribe(url: string): Promise<void> {
    if (busy) return;
    busy = true;
    error = null;
    try {
      const podcast = await feedSubscribe(url);
      await reload();
      await select(podcast.origin);
    } catch (e) {
      error = describe(e);
    } finally {
      busy = false;
    }
  }

  async function unsubscribe(podcast: Podcast): Promise<void> {
    const name = podcast.customTitle || podcast.title;
    if (!window.confirm(`确定退订《${name}》？它的单集和播放进度会一起删掉。`)) return;

    await feedUnsubscribe(podcast.origin).catch((e) => {
      error = describe(e);
    });

    if (selected === podcast.origin) {
      episodes = [];
      selected = null;
    }
    await reload();
    if (!selected && podcasts[0]) await select(podcasts[0].origin);
  }

  async function loadFavorites(): Promise<void> {
    favorites = new Set(await favoriteIds().catch(() => []));
  }

  async function toggleFavorite(episode: Episode): Promise<void> {
    if (favorites.has(episode.id)) {
      await favoriteRemove(episode.id).catch((e) => (error = describe(e)));
    } else {
      await favoriteAdd(episode.id).catch((e) => (error = describe(e)));
    }
    await loadFavorites();
  }

  async function startDownload(episode: Episode): Promise<void> {
    await downloadStart(episode.id).catch((e) => (error = describe(e)));
  }

  async function enqueue(episode: Episode): Promise<void> {
    await queueAdd(episode.id).catch((e) => {
      error = describe(e);
    });
    await player.refreshQueue();
    if (!showQueue) showQueue = true;
  }

  function openSettings(): void {
    if (!current) return;
    editTitle = current.customTitle || current.title;
    editIntro = current.skipIntroSec;
    editOutro = current.skipOutroSec;
    showSettings = !showSettings;
  }

  async function saveSettings(): Promise<void> {
    if (!current) return;
    await podcastUpdateSettings(current.origin, editTitle, editIntro, editOutro).catch((e) => {
      error = describe(e);
    });
    showSettings = false;
    await reload();
    if (selected) await select(selected);
  }

  async function refreshAll(): Promise<void> {
    if (refreshing) return;
    refreshing = true;
    refreshDone = 0;
    refreshTotal = 0;
    error = null;

    try {
      const summary = await feedRefreshAll(false);
      await reload();
      if (selected) await select(selected);

      const failed = summary.failed.length;
      error =
        failed > 0
          ? `${summary.total} 个源里 ${failed} 个没刷新成功`
          : `刷完 ${summary.total} 个源，新增 ${summary.totalNew} 集，用时 ${(summary.elapsedMs / 1000).toFixed(1)}s`;
    } catch (e) {
      error = describe(e);
    } finally {
      refreshing = false;
      refreshDone = 0;
      refreshTotal = 0;
    }
  }

  function describe(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return String(e);
  }
</script>

<div class="relative flex h-full flex-col bg-canvas">
  <FeedBar {busy} {error} onSubmit={subscribe} onOpenImport={() => (showImport = true)} />

  <div class="flex min-h-0 flex-1">
    <Sidebar
      {podcasts}
      {selected}
      {view}
      {refreshing}
      onView={(v) =>
        (view = v === "discover" ? "discover" : v === "library" ? "library" : "subscriptions")}
      onSelect={select}
      onUnsubscribe={unsubscribe}
      onRefreshAll={() => void refreshAll()}
    />

    {#if view === "discover"}
      <DiscoverPanel onSubscribed={() => void reload()} />
    {:else if view === "library"}
      <LibraryPanel />
    {:else}
    <main class="flex min-w-0 flex-1 flex-col">
      {#if current}
        <section class="flex shrink-0 gap-4 border-b border-line bg-surface px-4 py-3">
          {#if current.imageUrl}
            <img
              src={current.imageUrl}
              alt=""
              class="h-20 w-20 shrink-0 rounded-card border border-line object-cover"
            />
          {/if}
          <div class="min-w-0 flex-1">
            <h2 class="text-md font-semibold">{current.customTitle || current.title}</h2>
            {#if current.author}
              <p class="mt-0.5 text-xs text-ink-sub">{current.author}</p>
            {/if}
            <p class="mt-1 line-clamp-2 text-xs text-ink-weak">
              {stripHtml(current.description)}
            </p>
          </div>
          <div class="flex shrink-0 flex-col items-end gap-1">
            <p class="text-xs text-ink-weak">{current.episodeCount} 集</p>
            <button
              type="button"
              onclick={openSettings}
              class="h-7 rounded-pill border border-line px-2 text-[11px] text-ink-sub hover:bg-canvas hover:text-ink"
              >频道设置</button
            >
          </div>
        </section>
      {/if}

      {#if showSettings && current}
        <section class="shrink-0 border-b border-line bg-canvas px-4 py-3">
          <div class="flex flex-wrap items-end gap-3 text-xs">
            <label class="flex flex-col gap-1">
              <span class="text-ink-sub">频道名（改了不会被刷新覆盖）</span>
              <input
                bind:value={editTitle}
                class="h-8 w-56 rounded border border-line bg-surface px-2 text-xs"
              />
            </label>
            <label class="flex flex-col gap-1">
              <span class="text-ink-sub">跳过片头（秒）</span>
              <input
                type="number"
                min="0"
                max="3600"
                bind:value={editIntro}
                class="h-8 w-24 rounded border border-line bg-surface px-2 text-xs"
              />
            </label>
            <label class="flex flex-col gap-1">
              <span class="text-ink-sub">跳过片尾（秒）</span>
              <input
                type="number"
                min="0"
                max="3600"
                bind:value={editOutro}
                class="h-8 w-24 rounded border border-line bg-surface px-2 text-xs"
              />
            </label>
            <button
              type="button"
              onclick={() => void saveSettings()}
              class="h-8 rounded-pill bg-accent px-3 text-xs text-white">保存</button
            >
            <button
              type="button"
              onclick={() => (showSettings = false)}
              class="h-8 rounded-pill border border-line px-3 text-xs text-ink-sub">取消</button
            >
          </div>
        </section>
      {/if}

      <EpisodeList
        {episodes}
        {favorites}
        currentId={player.current?.id ?? null}
        onPlay={(ep) => void player.toggle(ep)}
        onQueue={(ep) => void enqueue(ep)}
        onFavorite={(ep) => void toggleFavorite(ep)}
        onDownload={(ep) => void startDownload(ep)}
      />
    </main>
    {/if}

    {#if showQueue}
      <QueueDrawer onClose={() => (showQueue = false)} />
    {/if}
  </div>

  {#if refreshing && refreshTotal > 0}
    <p class="shrink-0 border-t border-line bg-surface px-4 py-1 text-xs text-ink-sub">
      刷新中 {refreshDone}/{refreshTotal}
    </p>
  {/if}

  <PlayerBar
    queueOpen={showQueue}
    onToggleQueue={() => (showQueue = !showQueue)}
    onOpenPlayer={() => (showPlayerPage = true)}
  />

  {#if showPlayerPage}
    <PlayerPage onClose={() => (showPlayerPage = false)} />
  {/if}
</div>

{#if showImport}
  <ImportPanel
    onClose={() => (showImport = false)}
    onImported={() => {
      void reload();
      if (selected) void select(selected);
    }}
  />
{/if}

<audio bind:this={audioEl} preload="metadata" class="hidden"></audio>
