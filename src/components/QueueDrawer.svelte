<script lang="ts">
  import { queueClear, queueRemove, queueReorder } from "../lib/api/queue";
  import { formatDuration } from "../lib/format";
  import { player } from "../lib/player.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let dragId = $state<number | null>(null);
  let overId = $state<number | null>(null);

  async function remove(id: number): Promise<void> {
    await queueRemove(id).catch(() => undefined);
    await player.refreshQueue();
  }

  async function clear(): Promise<void> {
    if (player.queue.length === 0) return;
    if (!window.confirm("清空播放队列？")) return;
    await queueClear().catch(() => undefined);
    await player.refreshQueue();
  }

  /** 拖动排序：把 dragId 插到 overId 的位置 */
  async function drop(): Promise<void> {
    const from = dragId;
    const to = overId;
    dragId = null;
    overId = null;
    if (from === null || to === null || from === to) return;

    const ids = player.queue.map((q) => q.id);
    const fromIndex = ids.indexOf(from);
    if (fromIndex < 0) return;

    ids.splice(fromIndex, 1);
    ids.splice(ids.indexOf(to), 0, from);

    // 乐观更新：先把界面摆好，失败了再拉一次真实顺序
    const snapshot = player.queue;
    player.queue = ids
      .map((id) => snapshot.find((q) => q.id === id))
      .filter((q): q is NonNullable<typeof q> => q !== undefined);

    await queueReorder(ids).catch(() => undefined);
    await player.refreshQueue();
  }
</script>

<aside
  class="flex w-[320px] shrink-0 flex-col border-l border-line bg-surface"
  aria-label="播放队列"
>
  <header class="flex h-12 shrink-0 items-center justify-between border-b border-line px-4">
    <h2 class="text-sm font-semibold">播放队列</h2>
    <div class="flex items-center gap-1">
      <button
        type="button"
        onclick={() => void clear()}
        class="h-7 rounded-pill px-2 text-xs text-ink-sub hover:bg-canvas hover:text-ink"
        >清空</button
      >
      <button
        type="button"
        onclick={onClose}
        class="h-7 w-7 rounded-pill text-xs text-ink-sub hover:bg-canvas hover:text-ink"
        aria-label="收起队列">✕</button
      >
    </div>
  </header>

  <label
    class="flex h-9 shrink-0 items-center gap-2 border-b border-line px-4 text-xs text-ink-sub"
  >
    <input type="checkbox" bind:checked={player.autoAdvance} />
    播完自动接下一集
  </label>

  {#if player.queue.length === 0}
    <div class="flex flex-1 items-center justify-center px-6 text-center text-xs text-ink-weak">
      队列是空的。在单集上点「加入队列」，播完会自动接上。
    </div>
  {:else}
    <ul class="min-h-0 flex-1 overflow-y-auto">
      {#each player.queue as item, index (item.id)}
        <li
          class="flex items-center gap-2 border-b border-line px-3 py-2
                 {dragId === item.id ? 'opacity-40' : ''}
                 {overId === item.id && dragId !== item.id ? 'border-t-2 border-t-accent' : ''}"
          draggable="true"
          ondragstart={() => (dragId = item.id)}
          ondragover={(e) => {
            e.preventDefault();
            overId = item.id;
          }}
          ondrop={(e) => {
            e.preventDefault();
            void drop();
          }}
          ondragend={() => {
            dragId = null;
            overId = null;
          }}
        >
          <span class="w-5 shrink-0 text-center text-xs text-ink-weak tabular-nums">
            {index + 1}
          </span>

          <button
            type="button"
            onclick={() => void player.playQueueItem(item)}
            class="flex min-w-0 flex-1 items-center gap-2 text-left"
          >
            {#if item.imageUrl}
              <img
                src={item.imageUrl}
                alt=""
                class="h-8 w-8 shrink-0 rounded border border-line object-cover"
              />
            {:else}
              <div class="h-8 w-8 shrink-0 rounded border border-line bg-canvas"></div>
            {/if}
            <span class="min-w-0 flex-1">
              <span class="block truncate text-xs">{item.title || "(无标题)"}</span>
              <span class="block truncate text-[11px] text-ink-sub">
                {item.podcastTitle ?? "裸音频"}
                {item.durationSec ? ` · ${formatDuration(item.durationSec)}` : ""}
              </span>
            </span>
          </button>

          <button
            type="button"
            onclick={() => void remove(item.id)}
            class="h-7 w-7 shrink-0 rounded-pill text-xs text-ink-weak hover:bg-canvas hover:text-ink"
            aria-label="移出队列">✕</button
          >
        </li>
      {/each}
    </ul>
  {/if}
</aside>
