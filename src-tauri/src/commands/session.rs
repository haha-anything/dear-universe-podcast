use tauri::State;

use crate::{core::db, model::SessionDto};

/// 记住「正在听哪一集」，用来重启后恢复界面（真正的播放进度走 play_state）
#[tauri::command]
pub fn session_save(
    episode_id: Option<String>,
    position_sec: f64,
    speed: f64,
    volume: i32,
    db: State<'_, db::Database>,
) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::session::save(
        &conn,
        episode_id.as_deref(),
        to_ms(position_sec),
        speed,
        volume.clamp(0, 100),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn session_load(db: State<'_, db::Database>) -> Result<Option<SessionDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::session::load(&conn).map_err(|e| e.to_string())
}

fn to_ms(sec: f64) -> i64 {
    if !sec.is_finite() || sec <= 0.0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let ms = (sec * 1000.0) as i64;
    ms.max(0)
}
