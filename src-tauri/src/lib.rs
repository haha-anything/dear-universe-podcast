//! 大宇宙播客 —— 桌面端入口。
//!
//! M0 骨架：先保证窗口能起来、能打包。
//! 业务代码按里程碑往 commands/ 与 core/ 里加。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().init();

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("启动失败")
}
