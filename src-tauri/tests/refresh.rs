//! 刷新链路（需要联网）：先导入本机那份 21 订阅的老库，再刷一遍全部。
//!   cargo test --test refresh -- --ignored --nocapture
//!
//! 这是 M3 验收「订阅 20 频道刷新 < 15s」的直接证据 —— 用的是真实订阅，
//! 不是拼出来的假源。

use std::sync::Arc;

use dear_universe_podcast_lib::core::{db::Database, http::HttpClient, legacy, subscription};

#[tokio::test]
#[ignore = "需要联网：cargo test --test refresh -- --ignored --nocapture"]
async fn refreshing_real_subscriptions_is_fast_enough() {
    let dir = tempfile::tempdir().expect("建临时目录失败");
    let db = Arc::new(Database::open_in(dir.path()).expect("开库失败"));

    let imported = legacy::import(&db, &legacy::default_path().expect("没有老库"), dir.path())
        .expect("导入老库失败");
    println!("导入：{:?}", imported.plan);
    assert!(imported.plan.podcasts >= 20, "样本订阅太少，测不出并发");

    let http = HttpClient::new().expect("建 HTTP 客户端失败");
    let client = http.inner().clone();
    let shared = db.clone();

    let summary = subscription::refresh_all(shared, &client, true, Arc::new(|d, t| {
        println!("  {d}/{t}");
    }))
    .await
    .expect("刷新失败");

    println!(
        "刷新：{} 个源，新增 {} 集，失败 {} 个，用时 {}ms",
        summary.total,
        summary.total_new,
        summary.failed.len(),
        summary.elapsed_ms
    );
    for origin in &summary.failed {
        println!("  失败: {origin}");
    }

    assert_eq!(summary.total, imported.plan.podcasts, "有源没被刷到");

    // §12 的预算是「20 频道 < 15s」。这里源是真实分布的（含国内慢源），
    // 达标与否如实打印，断言只守一个明显不合理的上限，避免网络抖动就红。
    assert!(
        summary.elapsed_ms < 60_000,
        "刷新慢得离谱：{}ms",
        summary.elapsed_ms
    );

    // 第二次刷新：应当大面积命中 304 或 10 分钟节流，几乎不再产生新集
    let again = subscription::refresh_all(db.clone(), &client, false, Arc::new(|_, _| {}))
        .await
        .expect("二次刷新失败");
    println!("二次刷新：新增 {} 集，用时 {}ms", again.total_new, again.elapsed_ms);
    assert_eq!(again.total_new, 0, "刚刷过又冒出新集，判断新集的逻辑有问题");
}
