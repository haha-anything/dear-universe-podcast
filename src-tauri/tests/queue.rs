//! 队列行为测试（不联网）：排序、拖动重排、播完接下一集。
//!   cargo test --test queue

use dear_universe_podcast_lib::core::db::{self, Database};

const FEED: &str = "https://example.com/feed.xml";

/// 建一个库，塞两个节目各几集，再把前三集排进队列
fn seeded() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("建临时目录失败");
    let db = Database::open_in(dir.path()).expect("开库失败");

    {
        let conn = db.read().expect("拿连接失败");
        conn.execute(
            "INSERT INTO podcast (origin, title, created_at) VALUES (?1, 'p', 1)",
            [FEED],
        )
        .expect("插节目失败");

        for n in 1..=3 {
            let id = format!("{FEED}:a{n}");
            conn.execute(
                "INSERT INTO episode (id, origin, title, audio_url, pub_date)
                 VALUES (?1, ?2, ?3, 'https://a.mp3', ?4)",
                rusqlite::params![id, FEED, format!("第 {n} 集"), n],
            )
            .expect("插单集失败");

            db::queue::insert_at(&conn, Some(&id), None, "", 100 + n).expect("入队失败");
        }
    }

    (dir, db)
}

fn ids(db: &Database) -> Vec<i64> {
    let conn = db.read().expect("拿连接失败");
    db::queue::list(&conn)
        .expect("读队列失败")
        .iter()
        .map(|q| q.id)
        .collect()
}

fn titles(db: &Database) -> Vec<String> {
    let conn = db.read().expect("拿连接失败");
    db::queue::list(&conn)
        .expect("读队列失败")
        .iter()
        .map(|q| q.title.clone())
        .collect()
}

#[test]
fn queue_keeps_insertion_order() {
    let (_dir, db) = fresh_episodes();
    assert_eq!(titles(&db), vec!["第 1 集", "第 2 集", "第 3 集"]);
}

#[test]
fn reorder_changes_order_but_not_membership() {
    let (_dir, db) = fresh_episodes();
    let before = ids(&db);

    // 把第 3 集拖到最前
    let mut reordered = before.clone();
    let last = reordered.pop().expect("队列不该是空的");
    reordered.insert(0, last);

    {
        let conn = db.read().expect("拿连接失败");
        db::queue::reorder(&conn, &reordered).expect("重排失败");
    }

    assert_eq!(titles(&db), vec!["第 3 集", "第 1 集", "第 2 集"]);
    assert_eq!(ids(&db).len(), before.len(), "重排不该改变队列长度");
}

#[test]
fn next_after_walks_the_queue_and_wraps_to_none() {
    let (_dir, db) = fresh_episodes();
    let order = ids(&db);

    let second = {
        let conn = db.read().expect("拿连接失败");
        db::queue::next_after(&conn, order[0]).expect("查下一集失败")
    };
    assert_eq!(
        second.as_ref().map(|q| q.title.as_str()),
        Some("第 2 集"),
        "第一集的下一集应该是第二集"
    );

    let none = {
        let conn = db.read().expect("拿连接失败");
        db::queue::next_after(&conn, order[2]).expect("查下一集失败")
    };
    assert!(none.is_none(), "队尾之后不该还有下一集");
}

/// 不在队列里的集（用户直接从列表点播）也该能接到下一集 —— 从头开始
#[test]
fn next_after_unknown_episode_starts_from_head() {
    let (_dir, db) = fresh_episodes();

    let head = {
        let conn = db.read().expect("拿连接失败");
        db::queue::first(&conn).expect("读队首失败")
    };
    assert_eq!(head.as_ref().map(|q| q.title.as_str()), Some("第 1 集"));
}

#[test]
fn removing_episode_drops_it_from_queue() {
    let (_dir, db) = fresh_episodes();

    {
        let conn = db.read().expect("拿连接失败");
        db::queue::remove_episode(&conn, &format!("{FEED}:a2")).expect("出队失败");
    }

    assert_eq!(titles(&db), vec!["第 1 集", "第 3 集"]);
}

/// §8.5：裸 URL 项没有 episode 记录，列表里也得能读出来、能播
#[test]
fn raw_url_items_survive_the_join() {
    let (_dir, db) = fresh_episodes();

    {
        let conn = db.read().expect("拿连接失败");
        db::queue::insert_at(&conn, None, Some("https://raw.example/x.mp3"), "裸音频", 999)
            .expect("插入裸 URL 项失败");
    }

    let items = {
        let conn = db.read().expect("拿连接失败");
        db::queue::list(&conn).expect("读队列失败")
    };

    let raw = items
        .iter()
        .find(|q| q.episode_id.is_none())
        .expect("裸 URL 项被 LEFT JOIN 吞掉了");
    assert_eq!(raw.title, "裸音频");
    assert_eq!(raw.audio_url.as_deref(), Some("https://raw.example/x.mp3"));
}

fn fresh_episodes() -> (tempfile::TempDir, Database) {
    seeded()
}
