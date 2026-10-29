<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getVersion } from "@tauri-apps/api/app";
  import { theme, type ThemeMode } from "../lib/theme.svelte";
  import { checkForUpdate, installUpdate, type UpdateInfo } from "../lib/api/updater";
  import { opmlExport, opmlImport, opmlScan } from "../lib/api/opml";
  import { feedKeepLimit } from "../lib/api/discover";
  import { player } from "../lib/player.svelte";

  interface Props {
    onOpenImport: () => void;
    onOpmlImported: () => void;
  }

  let { onOpenImport, onOpmlImported }: Props = $props();

  const THEME_OPTIONS: ReadonlyArray<{ mode: ThemeMode; label: string }> = [
    { mode: "system", label: "跟随系统" },
    { mode: "light", label: "浅色" },
    { mode: "dark", label: "深色" },
  ];

  let version = $state("…");
  let keepLimit = $state(300);
  let checking = $state(false);
  let installing = $state(false);
  let update = $state<UpdateInfo | null>(null);
  let note = $state<string | null>(null);

  $effect(() => {
    getVersion().then((v) => (version = v)).catch(() => (version = "0.1.0"));
    feedKeepLimit().then((n) => (keepLimit = n)).catch(() => undefined);
  });

  async function doCheck(): Promise<void> {
    if (checking) return;
    checking = true;
    note = null;
    update = null;
    update = await checkForUpdate();
    checking = false;
    if (!update.available) note = "当前已是最新版本";
  }

  async function doInstall(): Promise<void> {
    if (installing) return;
    installing = true;
    try {
      await installUpdate();
    } catch (e) {
      note = `更新失败：${describe(e)}`;
      installing = false;
    }
  }

  // OPML 用和发现页一致的 file input + Blob 下载，不引入额外插件
  async function pickOpml(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    try {
      const text = await file.text();
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
      onOpmlImported();
    } catch (e) {
      note = `导入失败：${describe(e)}`;
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
      note = `导出失败：${describe(e)}`;
    }
  }

  function describe(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return String(e);
  }
</script>

<section class="min-h-0 flex-1 overflow-y-auto">
  <div class="mx-auto max-w-[640px] space-y-6 px-6 py-6">
    <h2 class="text-title font-semibold">设置</h2>

    {#if note}
      <p class="rounded-card border border-line bg-canvas px-3 py-2 text-xs text-ink-sub">{note}</p>
    {/if}

    <!-- 外观 -->
    <div class="space-y-2">
      <h3 class="text-sm font-medium">外观</h3>
      <p class="text-xs text-ink-weak">明暗双主题，默认跟随系统。</p>
      <div class="flex gap-2">
        {#each THEME_OPTIONS as opt (opt.mode)}
          <button
            type="button"
            onclick={() => theme.set(opt.mode)}
            class="h-8 rounded-pill border px-3 text-xs {theme.mode === opt.mode
              ? 'border-accent text-accent'
              : 'border-line text-ink-sub hover:text-ink'}">{opt.label}</button
          >
        {/each}
      </div>
    </div>

    <!-- 播放 -->
    <div class="space-y-2 border-t border-line pt-5">
      <h3 class="text-sm font-medium">播放</h3>
      <label class="flex items-center justify-between text-xs">
        <span>播完自动接下一集</span>
        <input
          type="checkbox"
          checked={player.autoAdvance}
          onchange={(e) => (player.autoAdvance = e.currentTarget.checked)}
          class="h-4 w-4 accent-[var(--du-accent)]"
        />
      </label>
      <p class="text-xs text-ink-weak">倍速 0.5–3.0、跳过片头片尾在播放条与频道设置里调。</p>
    </div>

    <!-- 订阅与存储 -->
    <div class="space-y-2 border-t border-line pt-5">
      <h3 class="text-sm font-medium">订阅与存储</h3>
      <p class="text-xs text-ink-sub">每个节目保留最新 {keepLimit} 集，刷新时自动清理过期单集（收藏 / 有进度 / 在队列里的会保留）。</p>
      <div class="flex flex-wrap gap-2 pt-1">
        <button
          type="button"
          onclick={onOpenImport}
          class="h-8 rounded-pill border border-line px-3 text-xs text-ink-sub hover:text-ink">从旧版 Podara 导入</button
        >
        <label
          class="h-8 cursor-pointer rounded-pill border border-line px-3 text-xs leading-8 text-ink-sub hover:text-ink"
        >
          导入 OPML
          <input type="file" accept=".opml,.xml" class="hidden" onchange={(e) => void pickOpml(e)} />
        </label>
        <button
          type="button"
          onclick={() => void exportOpml()}
          class="h-8 rounded-pill border border-line px-3 text-xs text-ink-sub hover:text-ink">导出 OPML</button
        >
      </div>
    </div>

    <!-- 更新 -->
    <div class="space-y-2 border-t border-line pt-5">
      <h3 class="text-sm font-medium">自动更新</h3>
      <p class="text-xs text-ink-weak">从 GitHub Releases 检查新版本。</p>
      <div class="flex items-center gap-2 pt-1">
        <button
          type="button"
          onclick={() => void doCheck()}
          disabled={checking}
          class="h-8 rounded-pill border border-line px-3 text-xs text-ink-sub hover:text-ink disabled:opacity-40"
          >{checking ? "检查中…" : "检查更新"}</button
        >
        {#if update?.available}
          <span class="text-xs text-ink-sub">发现新版本 {update.version}</span>
          <button
            type="button"
            onclick={() => void doInstall()}
            disabled={installing}
            class="h-8 rounded-pill bg-accent px-3 text-xs text-white disabled:opacity-40"
            >{installing ? "下载安装中…" : "下载并安装"}</button
          >
        {/if}
      </div>
    </div>

    <!-- 关于 -->
    <div class="space-y-2 border-t border-line pt-5">
      <h3 class="text-sm font-medium">关于</h3>
      <dl class="space-y-1 text-xs">
        <div class="flex justify-between">
          <dt class="text-ink-sub">版本</dt>
          <dd>{version}</dd>
        </div>
        <div class="flex justify-between">
          <dt class="text-ink-sub">许可证</dt>
          <dd>GPL-3.0</dd>
        </div>
        <div class="flex justify-between">
          <dt class="text-ink-sub">数据来源</dt>
          <dd>通用 RSS + Apple 公开目录</dd>
        </div>
      </dl>
      <button
        type="button"
        onclick={() => void openUrl("https://github.com/haha-anything/dear-universe-podcast")}
        class="h-8 rounded-pill border border-line px-3 text-xs text-ink-sub hover:text-ink">项目主页 ↗</button
      >
    </div>
  </div>
</section>
