//! 数据层的核心保证（不需要联网，CI 也能跑）：
//!   cargo test --test db_core
//!
//! 这里会直接写 SQL —— 只为验证 schema 行为（外键、级联、CHECK），
//! 业务代码里的 SQL 仍然只准出现在 src/core/db/ 下。

use dear_universe_podcast_lib::core::db::{self, Database};

fn fresh_db() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("建临时目录失败");
    let db = Database::open_in(dir.path()).expect("开库失败");
    (dir, db)
}

/// 注意：`db.read()` 拿的是互斥锁，**同一线程不能嵌套调用**——
/// 手里还握着 guard 的时候再来一次会直接死锁（第一个版本就踩了）。
/// 所以要么用 `count_on(&conn)`，要么先用块把 guard 放掉。
fn count(db: &Database, sql: &str) -> i64 {
    let conn = db.read().expect("拿连接失败");
    count_on(&conn, sql)
}

/// 已经有连接在手时用这个，避免重复加锁
fn count_on(conn: &rusqlite::Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).expect(sql)
}

#[test]
fn migration_runs_once_and_records_version() {
    let (dir, db) = fresh_db();

    let version: i64 = {
        let conn = db.read().expect("拿连接失败");
        conn.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .expect("查不到 schema_version")
    };
    assert_eq!(version, 1, "初始迁移必须落在 schema_version = 1");

    // 再开一次：迁移是幂等的，不能把已有数据搞坏
    drop(db);
    let reopened = Database::open_in(dir.path()).expect("重开库失败");
    assert_eq!(count(&reopened, "SELECT COUNT(*) FROM podcast"), 0);
    assert_eq!(
        count(&reopened, "SELECT COUNT(*) FROM schema_version"),
        1,
        "重复打开不该重复应用迁移"
    );
}

/// §8.3：SQLite 的 foreign_keys 默认是关的，不开就没有级联删除，
/// 参考实现因此攒了 1065 条孤儿进度。这条测试守着它。
#[test]
fn foreign_keys_are_really_on() {
    let (_dir, db) = fresh_db();
    let conn = db.read().expect("拿连接失败");

    let enabled: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .expect("读 PRAGMA 失败");
    assert_eq!(enabled, 1, "foreign_keys 必须是 ON");

    // 插一个 origin 不存在的单集，应当被外键拦下
    let result = conn.execute(
        "INSERT INTO episode (id, origin, title, audio_url, pub_date)
         VALUES ('x:1', 'https://缺失的播客/feed.xml', 't', 'https://a.mp3', 0)",
        [],
    );
    assert!(result.is_err(), "外键没生效：孤儿单集被写进去了");
}

#[test]
fn deleting_podcast_cascades_to_episodes_and_progress() {
    let (_dir, db) = fresh_db();
    {
        let conn = db.read().expect("拿连接失败");
        conn.execute(
            "INSERT INTO podcast (origin, title, created_at) VALUES ('https://f/feed.xml', 't', 1)",
            [],
        )
        .expect("插入节目失败");
        conn.execute(
            "INSERT INTO episode (id, origin, title, audio_url, pub_date)
             VALUES ('https://f/feed.xml:a', 'https://f/feed.xml', 'e', 'https://a.mp3', 1)",
            [],
        )
        .expect("插入单集失败");
        db::play_state::save(&conn, "https://f/feed.xml:a", 10_000, 100_000)
            .expect("写进度失败");
    }

    assert_eq!(count(&db, "SELECT COUNT(*) FROM episode"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM play_state"), 1);

    {
        let conn = db.read().expect("拿连接失败");
        db::podcast::remove(&conn, "https://f/feed.xml").expect("删除失败");
    }

    assert_eq!(count(&db, "SELECT COUNT(*) FROM episode"), 0, "单集没被级联删掉");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM play_state"), 0, "进度没被级联删掉");
}

/// §8.7：finished 是粘性位 —— 听完一次之后重听也不会回到未听完
#[test]
fn finished_bit_is_sticky() {
    let (_dir, db) = fresh_db();
    let id = "https://f/feed.xml:a";
    {
        let conn = db.read().expect("拿连接失败");
        conn.execute(
            "INSERT INTO podcast (origin, title, created_at) VALUES ('https://f/feed.xml', 't', 1)",
            [],
        )
        .expect("插入节目失败");
        conn.execute(
            "INSERT INTO episode (id, origin, title, audio_url, pub_date)
             VALUES (?1, 'https://f/feed.xml', 'e', 'https://a.mp3', 1)",
            [id],
        )
        .expect("插入单集失败");

        db::play_state::save(&conn, id, 30_000, 100_000).expect("写进度失败");
        let mid = db::play_state::get(&conn, id).expect("读进度失败").expect("应该有进度");
        assert!(!mid.finished, "听一半不该算听完");

        db::play_state::save(&conn, id, 99_000, 100_000).expect("写进度失败");
        let nearly = db::play_state::get(&conn, id).expect("读进度失败").expect("应该有进度");
        assert!(nearly.finished, "听到 99% 该置 finished");

        // 回头重听
        db::play_state::save(&conn, id, 5_000, 100_000).expect("写进度失败");
        let replay = db::play_state::get(&conn, id).expect("读进度失败").expect("应该有进度");
        assert!(replay.finished, "finished 被回退了，粘性位失效");
        assert_eq!(replay.position_sec, 5, "位置还是要跟着走");
    }
}

/// §8.5：队列项允许裸 URL，但不允许两列都空
#[test]
fn queue_item_requires_either_episode_or_url() {
    let (_dir, db) = fresh_db();
    {
        let conn = db.read().expect("拿连接失败");

        db::queue::insert_at(&conn, None, Some("https://raw.mp3"), "裸音频", 1)
            .expect("裸 URL 的队列项应该能插");
        assert_eq!(count_on(&conn, "SELECT COUNT(*) FROM queue_item"), 1);

        let denied = db::queue::insert_at(&conn, None, None, "两个都空", 1);
        assert!(denied.is_err(), "CHECK 约束没拦住两列都空的队列项");
    }
}
