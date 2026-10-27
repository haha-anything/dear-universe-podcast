//! 大宇宙播客 —— 桌面端入口。
//!
//! 分层：
//!   commands/ 命令层，只做校验和转发
//!   core/     业务逻辑（HTTP、RSS 解析）
//!   model.rs  跨 IPC 的 DTO

// 都 pub：集成测试（tests/）要从 crate 外部调用
pub mod commands;
pub mod core;
pub mod error;
pub mod model;

use tracing_subscriber::EnvFilter;

use commands::feed::feed_preview;
use core::http::HttpClient;

pub fn run() {
    init_logging();

    tracing::info!("启动");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![feed_preview])
        .setup(|app| {
            use tauri::Manager;
            app.manage(HttpClient::new()?);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动失败")
}

fn init_logging() {
    let filter = EnvFilter::builder()
        .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
