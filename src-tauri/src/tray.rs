//! 托盘。
//!
//! M0 就要能验证「最小化到托盘后音频不断」，所以这里是真接上的：
//! 关窗 → 隐藏到托盘；点托盘 / 菜单 → 恢复窗口。

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager, Result,
};

pub const TRAY_ID: &str = "main";

pub fn setup(app: &App) -> Result<()> {
    let show = MenuItemBuilder::with_id("show", "打开窗口").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

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
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => reveal(app),
            "quit" => app.exit(0),
            _ => {}
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
