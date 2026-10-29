import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateInfo {
  available: boolean;
  version: string | null;
  notes: string | null;
}

/** 检查 GitHub Releases 上是否有新版本（§7 M5 自动更新）。
 * 不可达 / 无发布时返回 available=false，绝不阻塞启动。 */
export async function checkForUpdate(): Promise<UpdateInfo> {
  try {
    const update = await check();
    if (update) {
      return {
        available: true,
        version: update.version,
        notes: update.body ?? null,
      };
    }
    return { available: false, version: null, notes: null };
  } catch {
    return { available: false, version: null, notes: null };
  }
}

/** 下载并安装更新，装完重启应用。调用方负责在 UI 上确认。 */
export async function installUpdate(): Promise<void> {
  const update = await check();
  if (!update) return;
  await update.downloadAndInstall();
  await relaunch();
}
