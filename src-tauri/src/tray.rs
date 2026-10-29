//! 托盘。
//!
//! M0 就要能验证「最小化到托盘后音频不断」，所以这里是真接上的：
//! 关窗 → 隐藏到托盘；点托盘 / 菜单 → 恢复窗口。

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Emitter, Manager, Result,
};

pub const TRAY_ID: &str = "main";

pub fn setup(app: &App) -> Result<()> {
    let show = MenuItemBuilder::with_id("show", "打开窗口").build(app)?;
    let play = MenuItemBuilder::with_id("play", "播放 / 暂停").build(app)?;
    let prev = MenuItemBuilder::with_id("prev", "上一集").build(app)?;
    let next = MenuItemBuilder::with_id("next", "下一集").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    // 托盘媒体控制走 media-key 事件给前端，和 mediaSession 用同一套播放逻辑
    let menu = MenuBuilder::new(app)
        .items(&[&show, &play, &prev, &next, &quit])
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("缺少窗口图标，检查 tauri.conf.json 的 bundle.icon");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("大宇宙播客")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                reveal(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let action = match event.id().as_ref() {
                "show" => {
                    reveal(app);
                    None
                }
                "play" => Some("play"),
                "prev" => Some("prev"),
                "next" => Some("next"),
                "quit" => {
                    app.exit(0);
                    None
                }
                _ => None,
            };
            if let Some(a) = action {
                let _ = app.emit("media-key", serde_json::json!({ "action": a }));
            }
        })
        .build(app)?;

    Ok(())
}

/// 显示 MainWindow 并聚焦
fn reveal<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
