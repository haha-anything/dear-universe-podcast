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
    discover::{
        discover_search, discover_top, feed_keep_limit, feed_mark_all_played, feed_refresh,
        feed_refresh_all,
    },
    download::{
        download_cancel, download_list, download_path, download_pause, download_resume,
        download_set_limit, download_start,
    },
    episode::{episode_get, episode_list},
    feed::{feed_list, feed_subscribe, feed_unsubscribe, podcast_update_settings},
    legacy::{legacy_default_path, legacy_import, legacy_scan},
    library::{
        favorite_add, favorite_ids, favorite_list, favorite_remove, history_clear, history_list,
        history_record, history_remove, progress_list_recent,
    },
    opml::{opml_export, opml_import, opml_scan},
    progress::{progress_get, progress_save},
    queue::{
        queue_add, queue_clear, queue_list, queue_next, queue_remove, queue_remove_episode,
        queue_reorder,
    },
    session::{session_load, session_save},
};
use core::{
    db::{self, Database},
    download,
    http::HttpClient,
    sources::apple::AppleClient,
};

pub fn run() {
    init_logging();

    tracing::info!("启动");

    tauri::Builder::default()
        // 单实例锁：第二个实例启动时聚焦已有窗口，避免抢同一个 DB（§18.5）
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            feed_subscribe,
            feed_list,
            feed_unsubscribe,
            feed_refresh,
            feed_refresh_all,
            feed_mark_all_played,
            feed_keep_limit,
            podcast_update_settings,
            discover_top,
            discover_search,
            favorite_list,
            favorite_add,
            favorite_remove,
            favorite_ids,
            history_list,
            history_record,
            history_remove,
            history_clear,
            progress_list_recent,
            download_list,
            download_start,
            download_pause,
            download_resume,
            download_cancel,
            download_path,
            download_set_limit,
            opml_scan,
            opml_import,
            opml_export,
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

            let db = std::sync::Arc::new(Database::open_in(&data_dir)?);
            app.manage(db.clone());
            app.manage(db::Shared(db));
            app.manage(HttpClient::new()?);
            app.manage(download::Downloader::new(&data_dir)?);
            // 目录服务起不来不能拖累启动：失败就退化成「发现页为空」
            match AppleClient::new() {
                Ok(client) => {
                    app.manage(client);
                }
                Err(e) => tracing::warn!(error = %e, "Apple 目录客户端初始化失败，发现页将为空"),
            }
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
