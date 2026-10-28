<script lang="ts">
  interface Props {
    busy: boolean;
    error: string | null;
    onSubmit: (url: string) => void;
    onOpenImport: () => void;
  }

  let { busy, error, onSubmit, onOpenImport }: Props = $props();

  let url = $state("");

  // 几个随手能用上的示例源，省得每次去找 feed 地址
  const SAMPLES: ReadonlyArray<{ label: string; url: string }> = [
    { label: "Life Kit", url: "https://feeds.npr.org/510338/podcast.xml" },
    { label: "Planet Money", url: "https://feeds.npr.org/510289/podcast.xml" },
    { label: "Lex Fridman", url: "https://lexfridman.com/feed/podcast/" },
  ];

  function submit(target: string): void {
    if (busy || target.trim().length === 0) return;
    onSubmit(target.trim());
  }
</script>

<header
  class="flex shrink-0 flex-wrap items-center gap-2 border-b border-line bg-surface px-4 py-3"
>
  <h1 class="mr-2 text-md font-semibold whitespace-nowrap">大宇宙播客</h1>

  <form
    class="flex min-w-[280px] flex-1 items-center gap-2"
    onsubmit={(e) => {
      e.preventDefault();
      submit(url);
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
      disabled={busy || url.trim().length === 0}
      class="h-9 shrink-0 rounded-pill bg-accent px-4 text-sm font-medium text-white disabled:opacity-40"
    >
      {busy ? "订阅中…" : "订阅"}
    </button>
  </form>

  <div class="flex items-center gap-1">
    {#each SAMPLES as sample (sample.url)}
      <button
        type="button"
        onclick={() => {
          url = sample.url;
          submit(sample.url);
        }}
        class="rounded-pill border border-line px-2.5 py-1 text-xs text-ink-sub hover:text-ink"
      >
        {sample.label}
      </button>
    {/each}
    <button
      type="button"
      onclick={onOpenImport}
      class="ml-1 rounded-pill border border-accent px-2.5 py-1 text-xs text-accent hover:bg-accent hover:text-white"
    >
      从旧版导入
    </button>
  </div>
</header>

{#if error}
  <p class="shrink-0 border-b border-line px-4 py-2 text-xs text-accent">{error}</p>
{/if}
