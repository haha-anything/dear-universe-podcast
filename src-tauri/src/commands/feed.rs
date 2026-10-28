use tauri::State;
use url::Url;

use crate::{
    core::{self, http::HttpClient},
    model::PodcastDto,
};

/// 订阅：抓一次 feed 并把节目 + 单集写进库。
/// 预览和订阅走同一条路，区别只在于要不要落盘 —— 这里选择落盘，
/// 因为「先看看再决定」这个场景用户可以在订阅后退订。
#[tauri::command]
pub async fn feed_subscribe(
    url: String,
    http: State<'_, HttpClient>,
    db: State<'_, core::db::Database>,
) -> Result<PodcastDto, String> {
    let target = validate_url(&url)?;
    let client = (*http).inner().clone();
    core::subscription::subscribe(&db, &client, &target)
        .await
        .map_err(|e| e.to_string())
}

/// 订阅列表（简称 = 全部已订阅的节目）
#[tauri::command]
pub fn feed_list(db: State<'_, core::db::Database>) -> Result<Vec<PodcastDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    core::db::podcast::list(&conn).map_err(|e| e.to_string())
}

/// 退订。级联删除单集与进度（外键 + ON DELETE CASCADE）
#[tauri::command]
pub fn feed_unsubscribe(origin: String, db: State<'_, core::db::Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    core::db::podcast::remove(&conn, &origin).map_err(|e| e.to_string())?;
    Ok(())
}

/// 校验 RSS 地址：必须是 http/https 的完整 URL
fn validate_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("地址不能为空".to_string());
    }

    // 用户经常只粘域名，补个 https 也救不了所有情况，明确报错让他补全更省事
    let parsed = Url::parse(trimmed)
        .map_err(|e| format!("这不是一个合法的 URL：{e}（记得带上 http:// 或 https://）"))?;

    match parsed.scheme() {
        "http" | "https" => Ok(parsed.to_string()),
        other => Err(format!("不支持的协议 {other}，只支持 http / https")),
    }
}
