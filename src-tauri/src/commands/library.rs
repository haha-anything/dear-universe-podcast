use tauri::State;

use crate::{
    core::db::{self, Database},
    model::{EpisodeDto, FavoriteEntryDto, HistoryEntryDto},
};

// ── 收藏 ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn favorite_list(
    limit: Option<u32>,
    offset: Option<u32>,
    db: State<'_, Database>,
) -> Result<Vec<FavoriteEntryDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::favorite::list(
        &conn,
        i64::from(limit.unwrap_or(200)),
        i64::from(offset.unwrap_or(0)),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn favorite_add(episode_id: String, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::favorite::add(&conn, &episode_id, db::now_ms()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn favorite_remove(episode_id: String, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::favorite::remove(&conn, &episode_id).map_err(|e| e.to_string())?;
    Ok(())
}

/// 收藏的 id 全集。列表渲染时用它标星，不必为每一集打一次 IPC。
#[tauri::command]
pub fn favorite_ids(db: State<'_, Database>) -> Result<Vec<String>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT episode_id FROM favorite")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// ── 历史 ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn history_list(
    limit: Option<u32>,
    offset: Option<u32>,
    db: State<'_, Database>,
) -> Result<Vec<HistoryEntryDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::history::list(
        &conn,
        i64::from(limit.unwrap_or(200)),
        i64::from(offset.unwrap_or(0)),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_remove(id: i64, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::history::remove(&conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn history_clear(db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::history::clear(&conn).map_err(|e| e.to_string())?;
    Ok(())
}

/// 开播时记一笔。由前端在 `play` 事件里调。
#[tauri::command]
pub fn history_record(episode_id: String, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;

    let origin: Option<String> = conn
        .query_row(
            "SELECT origin FROM episode WHERE id = ?1",
            [&episode_id],
            |r| r.get(0),
        )
        .ok();

    // 单集不在库里（比如裸 URL）就不记，history.origin 是非空的
    let Some(origin) = origin else {
        return Ok(());
    };

    // 同一集 60 秒内只记一笔：暂停/续播/拖动进度会反复触发 play 事件，
    // 不去重会把历史刷成同一集的连续几十条。
    let recent: Option<i64> = conn
        .query_row(
            "SELECT MAX(played_at) FROM history WHERE episode_id = ?1",
            [&episode_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    if let Some(last) = recent {
        if db::now_ms() - last < 60_000 {
            return Ok(());
        }
    }

    db::history::insert(&conn, &episode_id, &origin, db::now_ms()).map_err(|e| e.to_string())?;
    Ok(())
}

// ── 继续听 ──────────────────────────────────────────────────────────

/// 听过但没听完的，按最近听过的排。首页「继续听」用。
#[tauri::command]
pub fn progress_list_recent(limit: Option<u32>, db: State<'_, Database>) -> Result<Vec<EpisodeDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} FROM episode e
             JOIN podcast p ON p.origin = e.origin
             JOIN play_state ps ON ps.episode_id = e.id
             WHERE ps.finished = 0 AND ps.position_ms > 0
             ORDER BY ps.updated_at DESC
             LIMIT ?1",
            db::episode::COLS
        ))
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([i64::from(limit.unwrap_or(20))], |row| {
            db::episode::row_to_dto(row, 0)
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
