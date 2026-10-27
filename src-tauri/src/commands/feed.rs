use tauri::State;
use url::Url;

use crate::{
    core::{self, http::HttpClient},
    model::FeedDto,
};

/// 拉取并解析一个 RSS 地址，返回预览（M0：不落库、不写盘）。
#[tauri::command]
pub async fn feed_preview(url: String, http: State<'_, HttpClient>) -> Result<FeedDto, String> {
    let target = validate_url(&url)?;

    // State 自带 inner()（拿的是托管的 HttpClient），所以要显式解引用到 HttpClient 本体，
    // 再把底层 Client clone 出来 —— clone 很便宜且能把 State 的引用从 await 点上挪走
    let client = (*http).inner().clone();

    core::feed::fetch_feed(&client, &target)
        .await
        .map_err(|e| e.to_string())
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
