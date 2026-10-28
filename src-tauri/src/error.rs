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

    #[error("数据库操作失败：{0}")]
    Database(#[from] rusqlite::Error),

    #[error("数据库连接被占用：{0}")]
    DbLock(String),

    #[error("读写文件失败：{0}")]
    Io(String),

    #[error("找不到文件：{0}")]
    FileMissing(String),

    #[error("这不是一个可用的数据源：{0}")]
    InvalidSource(String),

    #[error("内部错误：{0}")]
    Anyhow(#[from] anyhow::Error),
}
