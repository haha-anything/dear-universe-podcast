import { invoke } from "@tauri-apps/api/core";
import type { ImportReport, LegacyScanReport } from "../types";

/** 旧版 Podara 数据库的默认位置 */
export function legacyDefaultPath(): Promise<string | null> {
  return invoke<string | null>("legacy_default_path");
}

/** 只读扫一遍，先看清楚了再决定导不导 */
export function legacyScan(path: string): Promise<LegacyScanReport> {
  return invoke<LegacyScanReport>("legacy_scan", { path });
}

export function legacyImport(path: string): Promise<ImportReport> {
  return invoke<ImportReport>("legacy_import", { path });
}
