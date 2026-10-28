//! 大宇宙播客 —— 桌面端入口。
//!
//! 分层：
//!   commands/ 命令层，只做校验和转发
//!   core/     业务逻辑（HTTP、RSS 解析、旧库导入）
//!   core/db/  所有 SQL
//!   model.rs  跨 IPC 的 DTO
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

use commands::{
    episode::{episode_get, episode_list},
    feed::{feed_list, feed_subscribe, feed_unsubscribe, podcast_update_settings},
    legacy::{legacy_default_path, legacy_import, legacy_scan},
    progress::{progress_get, progress_save},
    queue::{
        queue_add, queue_clear, queue_list, queue_next, queue_remove, queue_remove_episode,
        queue_reorder,
    },
    session::{session_load, session_save},
};
use core::{db::Database, http::HttpClient};

pub fn run() {
    init_logging();

    tracing::info!("启动");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            feed_subscribe,
            feed_list,
            feed_unsubscribe,
            podcast_update_settings,
            episode_list,
            episode_get,
            progress_save,
            progress_get,
            session_save,
            session_load,
            queue_list,
            queue_add,
            queue_remove,
            queue_remove_episode,
            queue_reorder,
            queue_clear,
            queue_next,
            legacy_default_path,
            legacy_scan,
            legacy_import,
        ])
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            let data_dir = app.path().app_data_dir()?;
            tracing::info!(dir = %data_dir.display(), "数据目录");

            app.manage(Database::open_in(&data_dir)?);
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
