use tauri::State;

use crate::{core::db, model::PlayStateDto};

/// 存播放进度。前端每 5 秒调一次 + 暂停/切集/退出前 flush。
#[tauri::command]
pub fn progress_save(
    episode_id: String,
    position_sec: f64,
    duration_sec: f64,
    db: State<'_, db::Database>,
) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::play_state::save(
        &conn,
        &episode_id,
        to_ms(position_sec),
        to_ms(duration_sec),
    )
    .map_err(|e| e.to_string())
}

/// 取某集的播放进度，用来「接着听」
#[tauri::command]
pub fn progress_get(
    episode_id: String,
    db: State<'_, db::Database>,
) -> Result<Option<PlayStateDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::play_state::get(&conn, &episode_id).map_err(|e| e.to_string())
}

fn to_ms(sec: f64) -> i64 {
    if !sec.is_finite() || sec <= 0.0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let ms = (sec * 1000.0) as i64;
    ms.max(0)
}
