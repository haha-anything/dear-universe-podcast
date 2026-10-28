use tauri::State;

use crate::{core::db, model::EpisodeDto};

/// 某个节目的单集，按发布时间倒序。
#[tauri::command]
pub fn episode_list(
    origin: String,
    limit: Option<u32>,
    offset: Option<u32>,
    db: State<'_, db::Database>,
) -> Result<Vec<EpisodeDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::episode::list_by_origin(&conn, &origin, i64::from(limit.unwrap_or(200)), i64::from(offset.unwrap_or(0)))
        .map_err(|e| e.to_string())
}

/// 按 id 取一集。重启后恢复「上次在听的那一集」时用。
#[tauri::command]
pub fn episode_get(id: String, db: State<'_, db::Database>) -> Result<Option<EpisodeDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::episode::get(&conn, &id).map_err(|e| e.to_string())
}
