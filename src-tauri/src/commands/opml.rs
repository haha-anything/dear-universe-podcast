use tauri::State;

use crate::{
    core::{db::Database, http::HttpClient, opml},
    model::OpmlImportReport,
};

/// 只读解析 OPML 文本，把里面有多少个源报给前端，**不联网**。
/// 前端先拿这个数给用户确认，再调 opml_import。
#[tauri::command]
pub fn opml_scan(text: String) -> Result<Vec<(String, String)>, String> {
    opml::read_feeds(&text).map_err(|e| e.to_string())
}

/// 把 OPML 里的源订阅进库。逐个订阅，失败的记录下来继续下一个。
#[tauri::command]
pub async fn opml_import(
    text: String,
    http: State<'_, HttpClient>,
    db: State<'_, Database>,
) -> Result<OpmlImportReport, String> {
    let feeds = opml::read_feeds(&text).map_err(|e| e.to_string())?;
    let client = (*http).inner().clone();

    opml::import_feeds(&db, &client, &feeds)
        .await
        .map_err(|e| e.to_string())
}

/// 导出：返回 OPML 文本，交给前端触发下载
#[tauri::command]
pub fn opml_export(db: State<'_, Database>) -> Result<String, String> {
    opml::build(&db).map_err(|e| e.to_string())
}
