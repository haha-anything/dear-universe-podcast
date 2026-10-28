use std::sync::Arc;

use tauri::State;

use crate::{
    core::{
        db::{self, Database},
        http::HttpClient,
        sources::apple::AppleClient,
        subscription::{self, KEEP_EPISODES},
    },
    model::{PodcastPreviewDto, RefreshOutcome, RefreshSummary},
};

/// 榜单。country 默认 `cn`（§6.1：默认 us 会命中美国区）
#[tauri::command]
pub async fn discover_top(
    apple: State<'_, AppleClient>,
    country: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<PodcastPreviewDto>, String> {
    let country = normalize_country(country.as_deref());
    let limit = limit.unwrap_or(50).clamp(1, 100);

    apple.top(&country, limit).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn discover_search(
    query: String,
    apple: State<'_, AppleClient>,
    country: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<PodcastPreviewDto>, String> {
    let country = normalize_country(country.as_deref());
    let limit = limit.unwrap_or(30).clamp(1, 50);

    apple
        .search(&query, &country, limit)
        .await
        .map_err(|e| e.to_string())
}

/// 刷新单个节目
#[tauri::command]
pub async fn feed_refresh(
    origin: String,
    force: Option<bool>,
    http: State<'_, HttpClient>,
    db: State<'_, Database>,
) -> Result<RefreshOutcome, String> {
    let client = (*http).inner().clone();
    subscription::refresh(&db, &client, &origin, force.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

/// 刷新全部。进度通过 `refresh-progress` 事件推给前端。
///
/// 单个源失败不打断整体，失败地址在返回值的 `failed` 里。
#[tauri::command]
pub async fn feed_refresh_all(
    force: Option<bool>,
    app: tauri::AppHandle,
    http: State<'_, HttpClient>,
    shared: State<'_, db::Shared>,
) -> Result<RefreshSummary, String> {
    use tauri::Emitter;

    let client = (*http).inner().clone();
    let shared: Arc<Database> = shared.0.clone();

    let handle = app.clone();
    let on_progress = Arc::new(move |done: usize, total: usize| {
        let _ = handle.emit(
            "refresh-progress",
            serde_json::json!({ "done": done, "total": total }),
        );
    });

    subscription::refresh_all(shared, &client, force.unwrap_or(false), on_progress)
        .await
        .map_err(|e| e.to_string())
}

/// 「全部标为已播」
#[tauri::command]
pub fn feed_mark_all_played(origin: String, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    crate::core::db::episode::mark_all_played(&conn, &origin).map_err(|e| e.to_string())?;
    Ok(())
}

/// 每个节目保留的单集数，给设置页显示用（§13）
#[tauri::command]
pub fn feed_keep_limit() -> i64 {
    KEEP_EPISODES
}

fn normalize_country(input: Option<&str>) -> String {
    let code = input.unwrap_or("cn").trim().to_lowercase();
    // 只认这两个，别让人传个乱七八糟的拼进 URL
    if matches!(code.as_str(), "cn" | "us") {
        code
    } else {
        "cn".to_string()
    }
}
