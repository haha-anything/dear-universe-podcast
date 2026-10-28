//! 旧版 Podara 数据的 dry-run 与导入（任务书 §15）。

use std::path::PathBuf;

use tauri::{Manager, State};

use crate::{
    core::{db, legacy},
    model::{ImportReport, LegacyScanReport},
};

/// 老库的默认位置，省得用户还得去找
#[tauri::command]
pub fn legacy_default_path() -> Option<String> {
    legacy::default_path().map(|p| p.display().to_string())
}

/// 只读扫一遍，给出「会导入什么 / 会跳过什么」。不写任何东西。
#[tauri::command]
pub fn legacy_scan(path: String) -> Result<LegacyScanReport, String> {
    legacy::scan(&PathBuf::from(path)).map_err(|e| e.to_string())
}

/// 真导入。单事务写入，完成后在数据目录留下 import-report.json。
#[tauri::command]
pub fn legacy_import(
    path: String,
    db: State<'_, db::Database>,
    app: tauri::AppHandle,
) -> Result<ImportReport, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取不到数据目录：{e}"))?;

    legacy::import(&db, &PathBuf::from(path), &data_dir).map_err(|e| e.to_string())
}
