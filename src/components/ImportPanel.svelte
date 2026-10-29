<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { legacyDefaultPath, legacyImport, legacyScan } from "../lib/api/legacy";
  import type { ImportReport, LegacyScanReport } from "../lib/types";

  interface Props {
    onClose: () => void;
    onImported: () => void;
  }

  let { onClose, onImported }: Props = $props();

  let path = $state("");
  let scanning = $state(false);
  let importing = $state(false);
  let report = $state<LegacyScanReport | null>(null);
  let done = $state<ImportReport | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    // 打开时把默认路径填上，省得用户去找 ~/.podara
    if (path) return;
    legacyDefaultPath()
      .then((p) => {
        path = p ?? "";
      })
      .catch(() => undefined);
  });

  async function scan(): Promise<void> {
    if (!path || scanning || importing) return;
    scanning = true;
    error = null;
    done = null;
    try {
      report = await legacyScan(path);
    } catch (e) {
      report = null;
      error = describe(e);
    } finally {
      scanning = false;
    }
  }

  async function browse(): Promise<void> {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: "Podara 数据库", extensions: ["db"] }],
    }).catch(() => null);
    if (picked && typeof picked === "string") path = picked;
  }

  async function runImport(): Promise<void> {
    if (!path || importing) return;
    importing = true;
    error = null;
    try {
      done = await legacyImport(path);
      onImported();
    } catch (e) {
      error = describe(e);
    } finally {
      importing = false;
    }
  }

  function describe(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return String(e);
  }
</script>

<div
  class="absolute inset-0 z-20 flex items-center justify-center bg-black/20"
  role="dialog"
  aria-modal="true"
  aria-label="从旧版导入"
>
  <div class="w-[560px] rounded-card border border-line bg-surface p-4">
    <div class="flex items-center justify-between">
      <h2 class="text-md font-semibold">从旧版 Podara 导入</h2>
      <button
        type="button"
        onclick={onClose}
        class="h-7 w-7 rounded-pill text-sm text-ink-sub hover:bg-canvas hover:text-ink">×</button
      >
    </div>

    <p class="mt-1 text-xs text-ink-weak">
      老库只读打开，先给你看清楚要写什么，确认之后才动你的数据。
    </p>

    <div class="mt-3 flex gap-2">
      <input
        bind:value={path}
        spellcheck="false"
        placeholder="podara.db 的路径"
        class="h-9 min-w-0 flex-1 rounded-pill border border-line bg-canvas px-3 text-body outline-none focus:border-accent"
      />
      <button
        type="button"
        onclick={() => void browse()}
        disabled={scanning || importing}
        class="h-9 shrink-0 rounded-pill border border-line px-3 text-sm text-ink-sub hover:text-ink disabled:opacity-40"
      >
        浏览
      </button>
      <button
        type="button"
        onclick={scan}
        disabled={scanning || importing || path.length === 0}
        class="h-9 shrink-0 rounded-pill border border-line px-3 text-sm text-ink-sub hover:text-ink disabled:opacity-40"
      >
        {scanning ? "扫描中…" : "扫描"}
      </button>
    </div>

    {#if report && report.usable}
      <dl class="mt-3 space-y-1 rounded-card border border-line bg-canvas px-3 py-2 text-xs">
        <div class="flex justify-between">
          <dt class="text-ink-sub">将导入</dt>
          <dd>
            订阅 {report.plan.podcasts} · 单集 {report.plan.episodes} · 进度 {report.plan
              .playStates} · 历史 {report.plan.history} · 队列 {report.plan.queueItems}
          </dd>
        </div>
        <div class="flex justify-between">
          <dt class="text-ink-sub">将跳过</dt>
          <dd class="text-ink-weak">
            空进度 {report.skipped.emptyPlayState} · 孤儿进度 {report.skipped
              .orphanPlayState} · 孤儿历史 {report.skipped.orphanHistory}
          </dd>
        </div>
        <div class="flex justify-between">
          <dt class="text-ink-sub">库里原有</dt>
          <dd class="text-ink-weak">
            订阅 {report.source.podcasts} · 单集 {report.source.episodes} · 进度 {report.source
              .playStates}
          </dd>
        </div>
      </dl>

      <div class="mt-3 flex items-center gap-2">
        <button
          type="button"
          onclick={runImport}
          disabled={importing}
          class="h-9 rounded-pill bg-accent px-4 text-sm font-medium text-white disabled:opacity-40"
        >
          {importing ? "导入中…" : "确认导入"}
        </button>
        <button
          type="button"
          onclick={onClose}
          class="h-9 rounded-pill border border-line px-3 text-sm text-ink-sub hover:text-ink"
          >取消</button
        >
      </div>
    {:else if report}
      <p class="mt-3 text-xs text-accent">{report.reason ?? "这个文件用不了"}</p>
    {/if}

    {#if done}
      <p class="mt-3 rounded-card border border-line bg-canvas px-3 py-2 text-xs">
        已写入：订阅 {done.plan.podcasts} · 单集 {done.plan.episodes} · 进度 {done.plan
          .playStates} · 历史 {done.plan.history} · 队列 {done.plan.queueItems}
        <br />
        <span class="text-ink-weak">耗时 {done.elapsedMs}ms{done.reportPath ? ` · 报告已写入数据目录` : ""}</span>
      </p>
    {/if}

    {#if error}
      <p class="mt-3 text-xs text-accent">{error}</p>
    {/if}
  </div>
</div>
