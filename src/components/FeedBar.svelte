<script lang="ts">
  interface Props {
    url: string;
    loading: boolean;
    error: string | null;
    onSubmit: () => void;
  }

  let { url = $bindable(""), loading, error, onSubmit }: Props = $props();

  // 几个随手能用上的示例源，省得每次去找 feed 地址
  const SAMPLES: ReadonlyArray<{ label: string; url: string }> = [
    { label: "Life Kit", url: "https://feeds.npr.org/510338/podcast.xml" },
    { label: "Planet Money", url: "https://feeds.npr.org/510289/podcast.xml" },
    { label: "Lex Fridman", url: "https://lexfridman.com/feed/podcast/" },
  ];
</script>

<header
  class="flex shrink-0 flex-wrap items-center gap-2 border-b border-line bg-surface px-4 py-3"
>
  <h1 class="mr-2 text-md font-semibold whitespace-nowrap">大宇宙播客</h1>

  <form
    class="flex min-w-[280px] flex-1 items-center gap-2"
    onsubmit={(e) => {
      e.preventDefault();
      onSubmit();
    }}
  >
    <input
      bind:value={url}
      type="url"
      spellcheck="false"
      placeholder="粘贴 RSS 订阅地址，如 https://example.com/feed.xml"
      class="h-9 min-w-0 flex-1 rounded-pill border border-line bg-canvas px-3 text-body outline-none focus:border-accent"
    />
    <button
      type="submit"
      disabled={loading || url.trim().length === 0}
      class="h-9 shrink-0 rounded-pill bg-accent px-4 text-sm font-medium text-white disabled:opacity-40"
    >
      {loading ? "加载中…" : "加载"}
    </button>
  </form>

  <div class="flex items-center gap-1">
    {#each SAMPLES as sample (sample.url)}
      <button
        type="button"
        onclick={() => {
          url = sample.url;
          onSubmit();
        }}
        class="rounded-pill border border-line px-2.5 py-1 text-xs text-ink-sub hover:text-ink"
      >
        {sample.label}
      </button>
    {/each}
  </div>
</header>

{#if error}
  <p class="shrink-0 border-b border-line px-4 py-2 text-xs text-accent">{error}</p>
{/if}
