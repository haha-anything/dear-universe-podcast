use tauri::State;

use crate::{core::db, core::db::Database, model::QueueItemDto};

#[tauri::command]
pub fn queue_list(db: State<'_, Database>) -> Result<Vec<QueueItemDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::queue::list(&conn).map_err(|e| e.to_string())
}

/// 加进队尾。已经在队列里的不加第二遍（重复点「加入队列」不该堆重复项）。
#[tauri::command]
pub fn queue_add(episode_id: String, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;

    let already: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM queue_item WHERE episode_id = ?1",
            [&episode_id],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if already {
        return Ok(());
    }

    // 标题留空：列表展示时 JOIN episode 拿最新标题，队列里存一份会过期
    db::queue::insert_at(&conn, Some(&episode_id), None, "", db::now_ms())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn queue_remove(id: i64, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::queue::remove(&conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

/// 播完自动出队用：手里只有 episode_id（用户可能不是从队列里点的）
#[tauri::command]
pub fn queue_remove_episode(episode_id: String, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::queue::remove_episode(&conn, &episode_id).map_err(|e| e.to_string())?;
    Ok(())
}

/// 拖动排序：传进来的 id 序列就是新顺序
#[tauri::command]
pub fn queue_reorder(ids: Vec<i64>, db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::queue::reorder(&conn, &ids).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn queue_clear(db: State<'_, Database>) -> Result<(), String> {
    let conn = db.read().map_err(|e| e.to_string())?;
    db::queue::clear(&conn).map_err(|e| e.to_string())?;
    Ok(())
}

/// 当前这集播完之后该接哪一集。没有下一集返回 null。
#[tauri::command]
pub fn queue_next(current_episode: String, db: State<'_, Database>) -> Result<Option<QueueItemDto>, String> {
    let conn = db.read().map_err(|e| e.to_string())?;

    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM queue_item WHERE episode_id = ?1",
            [&current_episode],
            |r| r.get(0),
        )
        .ok();

    let next = match id {
        Some(id) => db::queue::next_after(&conn, id),
        None => db::queue::first(&conn),
    };

    next.map_err(|e| e.to_string())
}
