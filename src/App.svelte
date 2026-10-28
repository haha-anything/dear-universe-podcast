<script lang="ts">
  import EpisodeList from "./components/EpisodeList.svelte";
  import FeedBar from "./components/FeedBar.svelte";
  import ImportPanel from "./components/ImportPanel.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import { episodeGet, episodeList } from "./lib/api/episode";
  import { feedList, feedSubscribe, feedUnsubscribe } from "./lib/api/feed";
  import { sessionLoad } from "./lib/api/player";
  import { stripHtml } from "./lib/format";
  import { player } from "./lib/player.svelte";
  import type { Episode, Podcast } from "./lib/types";

  let podcasts = $state<Podcast[]>([]);
  let selected = $state<string | null>(null);
  let episodes = $state<Episode[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let showImport = $state(false);
  let audioEl = $state<HTMLAudioElement | null>(null);

  // 快速连点订阅列表时，晚到的旧结果不能覆盖新的
  let loadToken = 0;

  const current = $derived(podcasts.find((p) => p.origin === selected) ?? null);

  // 音频元素挂进来之后交给 player 管；返回的解绑函数由 $effect 负责调用
  $effect(() => {
    if (!audioEl) return;
    return player.attach(audioEl);
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

    if (podcasts[0]) await select(podcasts[0].origin);
    await restoreLastSession();
  }

  async function select(origin: string): Promise<void> {
    const token = ++loadToken;
    selected = origin;

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

  function describe(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return String(e);
  }
</script>

<div class="relative flex h-full flex-col bg-canvas">
  <FeedBar {busy} {error} onSubmit={subscribe} onOpenImport={() => (showImport = true)} />

  <div class="flex min-h-0 flex-1">
    <Sidebar {podcasts} {selected} onSelect={select} onUnsubscribe={unsubscribe} />

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
          <p class="shrink-0 text-xs text-ink-weak">{current.episodeCount} 集</p>
        </section>
      {/if}

      <EpisodeList
        {episodes}
        currentId={player.current?.id ?? null}
        onPlay={(ep) => void player.toggle(ep)}
      />
    </main>
  </div>

  <PlayerBar />
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
