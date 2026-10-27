//! 大宇宙播客 —— 桌面端入口。
//!
//! 分层：
//!   commands/ 命令层，只做校验和转发
//!   core/     业务逻辑（HTTP、RSS 解析）
//!   model/    跨 IPC 的 DTO
//!   tray.rs   系统集成

// 都 pub：集成测试（tests/）要从 crate 外部调用
pub mod commands;
pub mod core;
pub mod error;
pub mod model;
mod tray;

// Manager：提供 app.manage() / app.get_webview_window()
use tauri::{Manager, WindowEvent};
use tracing_subscriber::EnvFilter;

use commands::feed::feed_preview;
use core::http::HttpClient;

pub fn run() {
    init_logging();

    tracing::info!("启动");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![feed_preview])
        .setup(|app| {
            app.manage(HttpClient::new()?);
            tray::setup(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // 关窗 = 藏到托盘。绝不能 destroy：Webview 被销毁，正在播的音频就断了。
                // 见任务书 §10.2 / §14.9。
                let _ = window.hide();
                api.prevent_close();
            }
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
