<script lang="ts">
  import { discoverSearch, discoverTop } from "../lib/api/discover";
  import { feedSubscribe } from "../lib/api/feed";
  import { opmlExport, opmlImport, opmlScan } from "../lib/api/opml";
  import type { PodcastPreview } from "../lib/types";

  let {
    onSubscribed,
  }: { onSubscribed: () => void } = $props();

  let query = $state("");
  let country = $state("cn");
  let items = $state<PodcastPreview[]>([]);
  let loading = $state(false);
  /** Apple 不可达时要说清是目录服务的问题，而不是报错弹窗（§6.3） */
  let offline = $state(false);
  let note = $state<string | null>(null);
  let busyOrigin = $state<string | null>(null);

  async function loadTop(): Promise<void> {
    loading = true;
    offline = false;
    note = null;
    try {
      items = await discoverTop(country, 60);
      if (items.length === 0) offline = true;
    } catch (e) {
      items = [];
      offline = true;
      note = describe(e);
    } finally {
      loading = false;
    }
  }

  async function search(): Promise<void> {
    if (query.trim().length === 0) return;
    loading = true;
    offline = false;
    note = null;
    try {
      items = await discoverSearch(query.trim(), country, 30);
    } catch (e) {
      items = [];
      offline = true;
      note = describe(e);
    } finally {
      loading = false;
    }
  }

  async function subscribe(preview: PodcastPreview): Promise<void> {
    busyOrigin = preview.feedUrl;
    note = null;
    try {
      await feedSubscribe(preview.feedUrl);
      onSubscribed();
      note = `已订阅《${preview.title}》`;
    } catch (e) {
      note = `${preview.title}：${describe(e)}`;
    } finally {
      busyOrigin = null;
    }
  }

  async function pickOpml(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;

    const text = await file.text();
    loading = true;
    note = null;
    try {
      const feeds = await opmlScan(text);
      if (feeds.length === 0) {
        note = "这个 OPML 里没找到可用的订阅源";
        return;
      }
      if (!window.confirm(`这个 OPML 里有 ${feeds.length} 个源，要逐个订阅吗？`)) return;

      const report = await opmlImport(text);
      note = `导入完成：新增 ${report.imported} · 已存在 ${report.skipped}${
        report.errors.length > 0 ? ` · 失败 ${report.errors.length}` : ""
      }`;
      onSubscribed();
    } catch (e) {
      note = describe(e);
    } finally {
      loading = false;
    }
  }

  async function exportOpml(): Promise<void> {
    try {
      const xml = await opmlExport();
      const blob = new Blob([xml], { type: "text/x-opml;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "dear-universe-subscriptions.opml";
      a.click();
      URL.revokeObjectURL(url);
      note = "OPML 已导出";
    } catch (e) {
      note = describe(e);
    }
  }

  function describe(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return String(e);
  }

  $effect(() => {
    void loadTop();
  });
</script>

<section class="flex min-h-0 flex-1 flex-col">
  <div class="flex shrink-0 flex-wrap items-center gap-2 border-b border-line bg-surface px-4 py-3">
    <form
      class="flex min-w-[240px] flex-1 items-center gap-2"
      onsubmit={(e) => {
        e.preventDefault();
        void search();
      }}
    >
      <input
        bind:value={query}
        spellcheck="false"
        placeholder="搜索播客（走 Apple 公开目录）"
        class="h-9 min-w-0 flex-1 rounded-pill border border-line bg-canvas px-3 text-body outline-none focus:border-accent"
      />
      <button
        type="submit"
        disabled={loading}
        class="h-9 shrink-0 rounded-pill bg-accent px-4 text-sm font-medium text-white disabled:opacity-40"
        >搜索</button
      >
    </form>

    <div class="flex items-center gap-1">
      {#each ["cn", "us"] as code (code)}
        <button
          type="button"
          onclick={() => {
            country = code;
            void loadTop();
          }}
          class="h-8 rounded-pill border border-line px-2.5 text-xs {country === code
            ? 'bg-canvas text-ink'
            : 'text-ink-sub hover:text-ink'}">{code.toUpperCase()}</button
        >
      {/each}
    </div>

    <div class="flex items-center gap-2">
      <label
        class="h-8 cursor-pointer rounded-pill border border-line px-3 text-xs leading-8 text-ink-sub hover:text-ink"
      >
        导入 OPML
        <input type="file" accept=".opml,.xml" class="hidden" onchange={(e) => void pickOpml(e)} />
      </label>
      <button
        type="button"
        onclick={() => void exportOpml()}
        class="h-8 rounded-pill border border-line px-3 text-xs text-ink-sub hover:text-ink"
        >导出 OPML</button
      >
    </div>
  </div>

  {#if note}
    <p class="shrink-0 border-b border-line bg-canvas px-4 py-2 text-xs text-ink-sub">{note}</p>
  {/if}

  <div class="min-h-0 flex-1 overflow-y-auto p-4">
    {#if offline}
      <div class="rounded-card border border-line bg-canvas px-4 py-6 text-center text-xs text-ink-sub">
        暂时无法连接目录服务，你可以粘贴 RSS 链接或导入 OPML。
        <br />
        <span class="text-ink-weak">已订阅内容的刷新与播放完全不受影响。</span>
      </div>
    {:else if loading && items.length === 0}
      <p class="p-8 text-center text-sm text-ink-sub">载入中…</p>
    {:else}
      <ul class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(160px, 1fr))">
        {#each items as item (item.feedUrl + item.id)}
          <li class="rounded-card border border-line bg-surface p-2">
            {#if item.imageUrl}
              <img
                src={item.imageUrl}
                alt=""
                loading="lazy"
                class="mb-2 aspect-square w-full rounded-cover border border-line object-cover"
              />
            {:else}
              <div
                class="mb-2 aspect-square w-full rounded-cover border border-line bg-canvas"
                aria-hidden="true"
              ></div>
            {/if}
            <p class="truncate text-xs font-medium" title={item.title}>{item.title}</p>
            <p class="mt-0.5 truncate text-[11px] text-ink-sub">{item.author ?? ""}</p>
            <button
              type="button"
              onclick={() => void subscribe(item)}
              disabled={busyOrigin === item.feedUrl}
              class="mt-2 h-7 w-full rounded-pill border border-line text-[11px] text-ink-sub hover:bg-canvas hover:text-ink disabled:opacity-40"
              >{busyOrigin === item.feedUrl ? "订阅中…" : "订阅"}</button
            >
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>
