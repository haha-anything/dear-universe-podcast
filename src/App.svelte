<script lang="ts">
  import FeedBar from "./components/FeedBar.svelte";
  import EpisodeList from "./components/EpisodeList.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import { feedPreview } from "./lib/api/feed";
  import { player } from "./lib/player.svelte";
  import { stripHtml } from "./lib/format";
  import type { FeedPreview as Feed } from "./lib/types";

  let url = $state("");
  let loading = $state(false);
  let error = $state<string | null>(null);
  let feed = $state<Feed | null>(null);
  let audioEl = $state<HTMLAudioElement | null>(null);

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

  async function load(): Promise<void> {
    const target = url.trim();
    if (!target || loading) return;

    loading = true;
    error = null;
    try {
      feed = await feedPreview(target);
    } catch (e) {
      feed = null;
      error = describe(e);
    } finally {
      loading = false;
    }
  }

  function describe(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return String(e);
  }
</script>

<main class="relative flex h-full flex-col bg-canvas">
  <FeedBar bind:url {loading} {error} onSubmit={load} />

  {#if feed}
    <section class="flex shrink-0 gap-4 border-b border-line bg-surface px-4 py-3">
      {#if feed.imageUrl}
        <img
          src={feed.imageUrl}
          alt=""
          class="h-20 w-20 shrink-0 rounded-card border border-line object-cover"
        />
      {/if}
      <div class="min-w-0 flex-1">
        <h2 class="text-md font-semibold">{feed.title}</h2>
        {#if feed.author}
          <p class="mt-0.5 text-xs text-ink-sub">{feed.author}</p>
        {/if}
        <p class="mt-1 line-clamp-2 text-xs text-ink-weak">{stripHtml(feed.description)}</p>
      </div>
      <p class="shrink-0 text-xs text-ink-weak">{feed.episodes.length} 集</p>
    </section>
  {/if}

  <EpisodeList
    episodes={feed?.episodes ?? []}
    currentId={player.current?.id ?? null}
    onPlay={(ep) => player.toggle(ep)}
  />

  <PlayerBar />
</main>

<audio bind:this={audioEl} preload="metadata" class="hidden"></audio>
