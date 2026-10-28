<script lang="ts">
  import type { Podcast } from "../lib/types";
  import { player } from "../lib/player.svelte";

  interface Props {
    podcasts: Podcast[];
    selected: string | null;
    onSelect: (origin: string) => void;
    onUnsubscribe: (podcast: Podcast) => void;
  }

  let { podcasts, selected, onSelect, onUnsubscribe }: Props = $props();

  function displayName(podcast: Podcast): string {
    return podcast.customTitle || podcast.title;
  }
</script>

<nav
  class="flex w-[220px] shrink-0 flex-col border-r border-line bg-surface"
  aria-label="订阅列表"
>
  <p class="flex h-9 shrink-0 items-center px-3 text-xs text-ink-sub">
    订阅 {podcasts.length}
  </p>

  <div class="min-h-0 flex-1 overflow-y-auto pb-2">
    {#if podcasts.length === 0}
      <p class="px-3 py-6 text-xs leading-relaxed text-ink-weak">
        还没有订阅。在上面粘一个 RSS 地址，或者从旧版 Podara 导入。
      </p>
    {:else}
      <ul>
        {#each podcasts as podcast (podcast.origin)}
          <li class="group relative">
            <button
              type="button"
              onclick={() => onSelect(podcast.origin)}
              class="flex w-full items-center gap-2 px-3 py-2 text-left {selected ===
              podcast.origin
                ? 'bg-canvas'
                : 'hover:bg-canvas'}"
            >
              {#if podcast.imageUrl}
                <img
                  src={podcast.imageUrl}
                  alt=""
                  loading="lazy"
                  class="h-8 w-8 shrink-0 rounded-cover border border-line object-cover"
                />
              {:else}
                <div
                  class="h-8 w-8 shrink-0 rounded-cover border border-line bg-canvas"
                  aria-hidden="true"
                ></div>
              {/if}

              <span class="min-w-0 flex-1">
                <span
                  class="block truncate text-xs {selected === podcast.origin
                    ? 'text-accent'
                    : 'text-ink'}">{displayName(podcast)}</span
                >
                <span class="block truncate text-[11px] text-ink-weak">
                  {podcast.episodeCount} 集{#if podcast.newCount > 0}· {podcast.newCount} 新{/if}
                </span>
              </span>

              {#if player.current?.origin === podcast.origin}
                <span class="h-1.5 w-1.5 shrink-0 rounded-pill bg-accent" title="正在播放"></span>
              {/if}
            </button>

            <button
              type="button"
              title="退订"
              onclick={() => onUnsubscribe(podcast)}
              class="absolute top-1.5 right-1.5 hidden h-5 w-5 items-center justify-center rounded-pill border border-line bg-surface text-xs text-ink-sub hover:text-accent group-hover:flex"
            >
              ×
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</nav>
