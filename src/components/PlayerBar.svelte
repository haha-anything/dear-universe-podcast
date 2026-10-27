<script lang="ts">
  import { player, PLAYBACK_RATES } from "../lib/player.svelte";
  import { formatDuration } from "../lib/format";

  let dragging = $state(false);
  let dragRatio = $state(0);

  // 拖动期间用本地值，松手才真正 seek —— 否则每帧改 currentTime 会卡顿
  const shownRatio = $derived.by(() => {
    if (dragging) return dragRatio;
    return player.durationSec > 0 ? player.positionSec / player.durationSec : 0;
  });
  const shownSec = $derived(shownRatio * player.durationSec);

  function ratioFrom(event: PointerEvent, track: HTMLElement): number {
    const rect = track.getBoundingClientRect();
    if (rect.width === 0) return 0;
    return Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
  }

  function cycleRate(): void {
    const next = PLAYBACK_RATES[(PLAYBACK_RATES.indexOf(player.rate as never) + 1) % PLAYBACK_RATES.length];
    player.setRate(next ?? 1);
  }
</script>

<footer
  class="flex h-[68px] shrink-0 items-center gap-3 border-t border-line bg-surface px-4 select-none"
>
  {#if player.current?.imageUrl}
    <img
      src={player.current.imageUrl}
      alt=""
      class="h-11 w-11 shrink-0 rounded-cover border border-line object-cover"
    />
  {:else}
    <div class="h-11 w-11 shrink-0 rounded-cover border border-line bg-canvas"></div>
  {/if}

  <div class="w-56 shrink-0">
    <p class="truncate text-sm font-medium">{player.current?.title ?? "未在播放"}</p>
    <p class="truncate text-xs text-ink-sub">{player.current?.podcastTitle ?? "选一集开始听"}</p>
  </div>

  <div class="flex flex-1 items-center gap-3">
    <span class="w-11 shrink-0 text-right text-xs text-ink-sub tabular-nums">
      {formatDuration(shownSec)}
    </span>
    <div
      role="slider"
      tabindex="0"
      aria-label="播放进度"
      aria-valuemin="0"
      aria-valuemax={Math.max(player.durationSec, 0)}
      aria-valuenow={Math.round(shownSec)}
      class="group flex h-5 flex-1 cursor-pointer items-center"
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
      onkeydown={(e) => {
        if (e.key === "ArrowLeft") player.skip(-15);
        if (e.key === "ArrowRight") player.skip(30);
      }}
    >
      <div class="h-1 w-full rounded-pill bg-track">
        <div
          class="h-1 rounded-pill bg-accent"
          style="width: {(shownRatio * 100).toFixed(3)}%"
        ></div>
      </div>
    </div>
    <span class="w-11 shrink-0 text-xs text-ink-sub tabular-nums">
      {formatDuration(player.durationSec)}
    </span>
  </div>

  <div class="flex shrink-0 items-center gap-1">
    <button
      type="button"
      onclick={() => player.skip(-15)}
      class="h-8 rounded-pill px-2 text-xs text-ink-sub hover:bg-canvas hover:text-ink"
      title="后退 15 秒">-15s</button
    >

    <button
      type="button"
      onclick={() => player.current && player.toggle(player.current)}
      disabled={!player.current}
      class="flex h-9 w-9 items-center justify-center rounded-pill bg-accent text-sm text-white disabled:opacity-30"
      title={player.playing ? "暂停" : "播放"}
    >
      {#if player.buffering}
        <span class="text-xs">…</span>
      {:else if player.playing}
        <span>❚❚</span>
      {:else}
        <span>▶</span>
      {/if}
    </button>

    <button
      type="button"
      onclick={() => player.skip(30)}
      class="h-8 rounded-pill px-2 text-xs text-ink-sub hover:bg-canvas hover:text-ink"
      title="前进 30 秒">+30s</button
    >

    <button
      type="button"
      onclick={cycleRate}
      class="ml-1 h-8 w-14 rounded-pill border border-line text-xs tabular-nums text-ink-sub hover:text-ink"
      title="切换倍速">{player.rate}×</button
    >
  </div>
</footer>

{#if player.error}
  <p class="absolute bottom-[68px] left-0 w-full bg-surface px-4 py-1.5 text-xs text-accent">
    {player.error}
  </p>
{/if}
