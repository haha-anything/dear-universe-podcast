<script lang="ts">
  import { fade } from "svelte/transition";
  import { formatDuration } from "../lib/format";
  import { player, PLAYBACK_RATES } from "../lib/player.svelte";
  import { stripHtml } from "../lib/format";

  let { onClose }: { onClose: () => void } = $props();

  let dragging = $state(false);
  let dragRatio = $state(0);

  const ratio = $derived.by(() => {
    if (dragging) return dragRatio;
    return player.durationSec > 0 ? player.positionSec / player.durationSec : 0;
  });

  function ratioFrom(event: PointerEvent, track: HTMLElement): number {
    const rect = track.getBoundingClientRect();
    if (rect.width === 0) return 0;
    return Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
  }
</script>

<div
  class="absolute inset-0 z-20 flex flex-col bg-canvas"
  transition:fade={{ duration: 180 }}
  role="dialog"
  aria-label="播放页"
>
  <header class="flex h-12 shrink-0 items-center justify-between px-4">
    <button
      type="button"
      onclick={onClose}
      class="h-8 rounded-pill px-3 text-xs text-ink-sub hover:bg-surface hover:text-ink"
      >收起</button
    >
    <span class="truncate px-4 text-xs text-ink-sub">{player.current?.podcastTitle ?? ""}</span>
    <span class="w-14"></span>
  </header>

  <div class="flex min-h-0 flex-1 flex-col items-center px-8 pb-4">
    {#if player.current?.imageUrl}
      <img
        src={player.current.imageUrl}
        alt=""
        class="max-h-[55vh] w-auto max-w-[420px] rounded-card border border-line object-contain"
      />
    {:else}
      <div class="h-[42vh] w-[420px] rounded-card border border-line bg-surface"></div>
    {/if}

    <h1 class="mt-5 max-w-[640px] text-center text-[20px] font-semibold leading-snug">
      {player.current?.title ?? "未在播放"}
    </h1>

    <div class="mt-5 flex w-full max-w-[640px] items-center gap-3">
      <span class="w-12 shrink-0 text-right text-xs text-ink-sub tabular-nums">
        {formatDuration(ratio * player.durationSec)}
      </span>
      <div
        role="slider"
        tabindex="0"
        aria-label="播放进度"
        aria-valuemin="0"
        aria-valuemax={Math.max(player.durationSec, 0)}
        aria-valuenow={Math.round(ratio * player.durationSec)}
        class="flex h-6 flex-1 cursor-pointer items-center"
        onpointerdown={(e) => {
          const track = e.currentTarget;
          track.setPointerCapture(e.pointerId);
          dragging = true;
          dragRatio = ratioFrom(e, track);
        }}
        onpointermove={(e) => {
          if (!dragging) return;
          dragRatio = ratioFrom(e, e.currentTarget);
        }}
        onpointerup={(e) => {
          if (!dragging) return;
          dragging = false;
          e.currentTarget.releasePointerCapture(e.pointerId);
          player.seek(dragRatio * player.durationSec);
        }}
      >
        <div class="h-1 w-full rounded-pill bg-track">
          <div class="h-1 rounded-pill bg-accent" style="width: {(ratio * 100).toFixed(3)}%"></div>
        </div>
      </div>
      <span class="w-12 shrink-0 text-xs text-ink-sub tabular-nums">
        {formatDuration(player.durationSec)}
      </span>
    </div>

    <div class="mt-4 flex items-center gap-2">
      <button
        type="button"
        onclick={() => player.skip(-30)}
        class="h-9 rounded-pill px-3 text-xs text-ink-sub hover:bg-surface hover:text-ink">-30s</button
      >
      <button
        type="button"
        onclick={() => player.skip(-15)}
        class="h-9 rounded-pill px-3 text-xs text-ink-sub hover:bg-surface hover:text-ink">-15s</button
      >
      <button
        type="button"
        onclick={() => player.current && player.toggle(player.current)}
        disabled={!player.current}
        class="flex h-11 w-11 items-center justify-center rounded-pill bg-accent text-white disabled:opacity-30"
      >
        {#if player.playing}<span>❚❚</span>{:else}<span>▶</span>{/if}
      </button>
      <button
        type="button"
        onclick={() => player.skip(30)}
        class="h-9 rounded-pill px-3 text-xs text-ink-sub hover:bg-surface hover:text-ink">+30s</button
      >
      <select
        value={player.rate}
        onchange={(e) => player.setRate(Number(e.currentTarget.value))}
        class="h-9 rounded-pill border border-line bg-surface px-2 text-xs text-ink-sub"
        aria-label="倍速"
      >
        {#each PLAYBACK_RATES as rate (rate)}
          <option value={rate}>{rate}×</option>
        {/each}
      </select>
    </div>

    {#if player.current?.description}
      <p class="mt-5 max-h-32 max-w-[640px] overflow-y-auto text-xs leading-relaxed text-ink-sub">
        {stripHtml(player.current.description)}
      </p>
    {/if}
  </div>
</div>
