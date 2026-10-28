<script lang="ts">
  import {
    Virtualizer,
    elementScroll,
    observeElementOffset,
    observeElementRect,
  } from "@tanstack/virtual-core";
  import type { Episode } from "../lib/types";
  import { formatDuration, formatRelative, stripHtml } from "../lib/format";

  interface Props {
    episodes: Episode[];
    currentId: string | null;
    onPlay: (episode: Episode) => void;
  }

  let { episodes, currentId, onPlay }: Props = $props();

  const ROW_HEIGHT = 84;

  let scrollEl = $state<HTMLDivElement | null>(null);

  // AGENTS.md：列表超过 200 条必须虚拟滚动。
  // 这里直接用 virtual-core 而不是 @tanstack/svelte-virtual：
  // 后者的 store 适配层在 Svelte 5 runes 下只把 count 写进去、不触发重新计算，
  // 实测 getTotalSize() 恒为 0。用 onChange 自增 tick 来驱动视图反而简单可靠。
  let tick = $state(0);

  const onChange = (): void => {
    tick += 1;
  };

  const virtualizer = new Virtualizer<HTMLDivElement, HTMLDivElement>({
    count: 0,
    getScrollElement: () => scrollEl,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
    observeElementRect,
    observeElementOffset,
    scrollToFn: elementScroll,
    onChange,
  });

  // virtual-core 的 setOptions 只用「传入的键」覆盖默认值，没传的必需项
  // （observeElementRect / observeElementOffset / scrollToFn / estimateSize）会被直接置空，
  // 表现为 this.options.observeElementRect is not a function。
  // 所以必须先把现有 options 摊开 —— 这一行不能省。
  $effect(() => {
    const count = episodes.length;
    const container = scrollEl;
    virtualizer.setOptions({
      ...virtualizer.options,
      count,
      getScrollElement: () => container,
      onChange,
    });
    virtualizer._willUpdate();
  });

  // _didMount 返回的就是清理函数，交给 effect 在卸载时调用
  $effect(() => virtualizer._didMount());

  const rows = $derived.by(() => {
    tick; // 订阅计算变更
    return virtualizer.getVirtualItems();
  });

  const totalSize = $derived.by(() => {
    tick;
    return virtualizer.getTotalSize();
  });

  /** 听过但没听完的，标一下听到哪了 */
  function progressLabel(ep: Episode): string {
    if (ep.finished) return "已听完";
    const done = ep.positionSec ?? 0;
    if (done <= 5) return "";
    const total = ep.durationSec ?? 0;
    if (total > 0 && done / total > 0.98) return "已听完";
    return `听到 ${formatDuration(done)}`;
  }
</script>

<div
  bind:this={scrollEl}
  class="min-h-0 flex-1 overflow-y-auto"
  data-rows={rows.length}
  data-total={totalSize}
  data-count={episodes.length}
>
  {#if episodes.length === 0}
    <p class="p-8 text-center text-sm text-ink-sub">还没有内容，先在上面填一个 RSS 地址</p>
  {:else}
    <div class="relative w-full" style="height: {totalSize}px">
      {#each rows as row (row.key)}
        {@const ep = episodes[row.index]}
        {#if ep}
          <div
            class="absolute top-0 left-0 w-full"
            style="height: {row.size}px; transform: translateY({row.start}px)"
          >
            <button
              type="button"
              onclick={() => onPlay(ep)}
              class="flex h-full w-full items-start gap-3 border-b border-line px-4 py-3 text-left {currentId ===
              ep.id
                ? 'bg-surface'
                : 'hover:bg-surface'}"
            >
              {#if ep.imageUrl}
                <img
                  src={ep.imageUrl}
                  alt=""
                  loading="lazy"
                  class="h-12 w-12 shrink-0 rounded-cover border border-line bg-canvas object-cover"
                />
              {:else}
                <div
                  class="h-12 w-12 shrink-0 rounded-cover border border-line bg-canvas"
                  aria-hidden="true"
                ></div>
              {/if}

              <div class="min-w-0 flex-1">
                <p
                  class="truncate text-sm font-medium {currentId === ep.id
                    ? 'text-accent'
                    : 'text-ink'}"
                >
                  {ep.title}
                </p>
                <p class="mt-0.5 truncate text-xs text-ink-sub">
                  {ep.podcastTitle} · {formatRelative(ep.publishedMs)} · {formatDuration(
                    ep.durationSec,
                  )}{#if progressLabel(ep)} · {progressLabel(ep)}{/if}
                </p>
                <p class="mt-0.5 line-clamp-1 text-xs text-ink-weak">
                  {stripHtml(ep.description)}
                </p>
              </div>
            </button>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>
