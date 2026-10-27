use thiserror::Error;

/// 库层错误。所有错误都能转成一句人话给 UI 显示，
/// 不许一路 unwrap 到崩溃（AGENTS.md 代码规范）。
#[derive(Debug, Error)]
pub enum AppError {
    #[error("网络请求失败：{0}")]
    Network(#[from] reqwest::Error),

    #[error("服务端返回 HTTP {status}")]
    Http { status: u16 },

    #[error("RSS 解析失败：{0}")]
    FeedParse(String),

    #[error("内部错误：{0}")]
    Anyhow(#[from] anyhow::Error),
}
