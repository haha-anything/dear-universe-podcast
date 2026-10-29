//! `download_task` 表的读写。
//!
//! **只存文件名，不存绝对路径**（§8.6）：数据目录会随版本 / 系统变，
//! 存绝对路径换个机器就全废了。

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::DownloadTaskDto;

const COLS: &str = "d.episode_id, d.origin, d.file_name, d.received, d.total, d.status, d.error, \
                    d.updated_at, e.title, e.image_url, e.duration_ms";

pub fn upsert(
    conn: &Connection,
    episode_id: &str,
    origin: &str,
    file_name: &str,
    status: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO download_task (episode_id, origin, file_name, status, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(episode_id) DO UPDATE SET
            file_name  = excluded.file_name,
            status     = excluded.status,
            error      = NULL,
            updated_at = excluded.updated_at",
        params![episode_id, origin, file_name, status, super::now_ms()],
    )?;
    Ok(())
}

pub fn set_status(
    conn: &Connection,
    episode_id: &str,
    status: &str,
    error: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE download_task SET status = ?2, error = ?3, updated_at = ?4 WHERE episode_id = ?1",
        params![episode_id, status, error, super::now_ms()],
    )?;
    Ok(())
}

pub fn set_progress(
    conn: &Connection,
    episode_id: &str,
    received: i64,
    total: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE download_task
            SET received = ?2, total = ?3, updated_at = ?4
          WHERE episode_id = ?1",
        params![episode_id, received, total, super::now_ms()],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, episode_id: &str) -> Result<Option<DownloadTaskDto>, rusqlite::Error> {
    conn.query_row(
        &format!(
            "SELECT {COLS} FROM download_task d
             JOIN episode e ON e.id = d.episode_id
             WHERE d.episode_id = ?1"
        ),
        [episode_id],
        to_dto,
    )
    .optional()
}

pub fn list(conn: &Connection) -> Result<Vec<DownloadTaskDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM download_task d
         JOIN episode e ON e.id = d.episode_id
         ORDER BY d.updated_at DESC"
    ))?;

    let mut rows = stmt.query_map([], to_dto)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next() {
        out.push(row?);
    }
    Ok(out)
}

/// 已完成的下载，用来播的时候优先走本地文件
pub fn completed_file(conn: &Connection, episode_id: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT file_name FROM download_task
          WHERE episode_id = ?1 AND status = 'done' AND file_name IS NOT NULL",
        [episode_id],
        |r| r.get(0),
    )
    .optional()
}

pub fn remove(conn: &Connection, episode_id: &str) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM download_task WHERE episode_id = ?1", [episode_id])
}

fn to_dto(row: &rusqlite::Row<'_>) -> Result<DownloadTaskDto, rusqlite::Error> {
    Ok(DownloadTaskDto {
        episode_id: row.get(0)?,
        origin: row.get(1)?,
        file_name: row.get(2)?,
        received: row.get(3)?,
        total: row.get(4)?,
        status: row.get(5)?,
        error: row.get(6)?,
        updated_at: row.get(7)?,
        title: row.get(8)?,
        image_url: row.get(9)?,
        duration_sec: super::episode::ms_to_sec(row.get(10)?),
    })
}
