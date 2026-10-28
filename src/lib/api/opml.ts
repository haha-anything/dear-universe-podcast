import { invoke } from "@tauri-apps/api/core";
import type { OpmlImportReport } from "../types";

/** 只读解析，返回 [feedUrl, 标题] 列表，不联网 */
export function opmlScan(text: string): Promise<Array<[string, string]>> {
  return invoke<Array<[string, string]>>("opml_scan", { text });
}

export function opmlImport(text: string): Promise<OpmlImportReport> {
  return invoke<OpmlImportReport>("opml_import", { text });
}

/** 导出：返回 OPML 文本，前端拿它触发下载 */
export function opmlExport(): Promise<string> {
  return invoke<string>("opml_export");
}
