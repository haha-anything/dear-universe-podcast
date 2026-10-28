//! 订阅与进度的持久化链路（需要联网拉 RSS）：
//!   cargo test --test subscription -- --ignored --nocapture
//!
//! M1 验收要的是「重启后订阅和进度都在」，这里用「关掉连接重开一次」模拟重启。

use dear_universe_podcast_lib::{
    core::{db, http::HttpClient, subscription},
    core::db::Database,
};

const NPR_LIFE_KIT: &str = "https://feeds.npr.org/510338/podcast.xml";

#[tokio::test]
#[ignore = "需要联网：cargo test --test subscription -- --ignored --nocapture"]
async fn subscription_and_progress_survive_reopen() {
    let dir = tempfile::tempdir().expect("建临时目录失败");

    let first_episode;
    let first_duration;

    {
        let db = Database::open_in(dir.path()).expect("开库失败");
        let http = HttpClient::new().expect("建 HTTP 客户端失败");
        let client = http.inner().clone();

        let podcast = subscription::subscribe(&db, &client, NPR_LIFE_KIT)
            .await
            .expect("订阅失败");
        println!("订阅到：{}（{} 集）", podcast.title, podcast.episode_count);
        assert!(podcast.episode_count > 0, "订阅之后库里应该有单集");

        let episodes = {
            let conn = db.read().expect("拿连接失败");
            db::episode::list_by_origin(&conn, &podcast.origin, 10, 0).expect("读单集失败")
        };
        assert!(!episodes.is_empty());

        first_episode = episodes[0].id.clone();
        first_duration = episodes[0].duration_sec.unwrap_or(0);

        // 假装听了一半
        {
            let conn = db.read().expect("拿连接失败");
            db::play_state::save(&conn, &first_episode, 60_000, 600_000).expect("写进度失败");
            db::session::save(&conn, Some(&first_episode), 60_000, 1.0, 100)
                .expect("写会话失败");
        }

        // 重复订阅同一个地址不应报错，也不该翻倍
        let again = subscription::subscribe(&db, &client, NPR_LIFE_KIT)
            .await
            .expect("重复订阅失败");
        assert_eq!(again.episode_count, podcast.episode_count, "重复订阅让单集翻倍了");
    }

    // ── 这里是「重启」 ───────────────────────────────────────────────
    let db = Database::open_in(dir.path()).expect("重开库失败");

    let list = {
        let conn = db.read().expect("拿连接失败");
        db::podcast::list(&conn).expect("读订阅列表失败")
    };
    println!("重启后订阅数：{}", list.len());
    assert_eq!(list.len(), 1, "订阅丢了");
    assert!(list[0].episode_count > 0);

    let progress = {
        let conn = db.read().expect("拿连接失败");
        db::play_state::get(&conn, &first_episode).expect("读进度失败")
    };
    let progress = progress.expect("进度丢了");
    println!("重启后进度：{}s / {}s", progress.position_sec, progress.duration_sec);
    assert_eq!(progress.position_sec, 60, "进度位置不对");
    assert_eq!(progress.duration_sec, 600, "进度时长不对");

    let session = {
        let conn = db.read().expect("拿连接失败");
        db::session::load(&conn).expect("读会话失败")
    };
    assert_eq!(
        session.as_ref().and_then(|s| s.current_episode.as_deref()),
        Some(first_episode.as_str()),
        "会话没记住上次在听哪一集"
    );

    // 单位核对：单集的时长从库里读回来应该和订阅时的秒数一致
    let episodes = {
        let conn = db.read().expect("拿连接失败");
        db::episode::list_by_origin(&conn, &list[0].origin, 5, 0).expect("读单集失败")
    };
    let stored = episodes
        .iter()
        .find(|e| e.id == first_episode)
        .expect("重开后找不到那集了");
    println!("单集时长（重启后）：{:?}s", stored.duration_sec);
    assert_eq!(stored.duration_sec.unwrap_or(0), first_duration, "往返一次后时长变了");

    // 退订 → 连带删干净
    {
        let conn = db.read().expect("拿连接失败");
        db::podcast::remove(&conn, &list[0].origin).expect("退订失败");
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM episode"), 0, "退订后单集还在");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM play_state"), 0, "退订后进度还在");
}

fn count(db: &Database, sql: &str) -> i64 {
    let conn = db.read().expect("拿连接失败");
    conn.query_row(sql, [], |r| r.get(0)).expect(sql)
}
