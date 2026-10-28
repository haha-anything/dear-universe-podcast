//! Apple 公开目录（需要联网）：榜单与搜索能不能用、不可达时会不会炸。
//!   cargo test --test discover -- --ignored --nocapture

use dear_universe_podcast_lib::core::sources::apple::AppleClient;

/// §6.1：country 默认 cn，参考实现的 bug 就是默认了 us
#[tokio::test]
#[ignore = "需要联网：cargo test --test discover -- --ignored --nocapture"]
async fn top_chart_returns_subscribable_feeds() {
    let apple = AppleClient::new().expect("建客户端失败");

    let top = apple.top("cn", 20).await.expect("榜单请求失败");
    println!("榜单 cn：{} 条", top.len());
    for item in top.iter().take(5) {
        println!("  {} — {}", item.title, item.feed_url);
    }

    // Apple 挂了要返回空列表而不是 Err（§6.3），所以这里不断言条数，
    // 只断言「拿到的每一条都真的能订」
    for item in &top {
        assert!(
            item.feed_url.starts_with("http"),
            "榜单给了不能订的地址：{}",
            item.feed_url
        );
    }
}

#[tokio::test]
#[ignore = "需要联网：cargo test --test discover -- --ignored --nocapture"]
async fn search_finds_chinese_podcasts() {
    let apple = AppleClient::new().expect("建客户端失败");

    let hits = apple.search("故事", "cn", 10).await.expect("搜索失败");
    println!("搜索「故事」：{} 条", hits.len());
    for item in hits.iter().take(5) {
        println!("  {} — {}", item.title, item.feed_url);
    }

    // 缓存：同一条件再搜一次应当直接命中，不打第二次网络请求
    let again = apple.search("故事", "cn", 10).await.expect("缓存命中失败");
    assert_eq!(again.len(), hits.len(), "缓存命中后结果变了");
}

/// Apple 不可达时必须返回空列表，绝不把错误抛给调用方（§6.3）
#[tokio::test]
#[ignore = "需要联网：cargo test --test discover -- --ignored --nocapture"]
async fn unreachable_catalog_returns_empty() {
    let apple = AppleClient::new().expect("建客户端失败");

    // 用一个不存在的国家码拼出不存在的地址，模拟目录服务挂掉
    let top = apple.top("zz", 10).await.expect("不该报错");
    assert!(top.is_empty(), "404 时不该有结果");

    let hits = apple.search("test", "zz", 10).await.expect("不该报错");
    assert!(hits.is_empty(), "异常国家码时不该有结果");

    // 顺带确认正常的那个确实能用，否则上面那条「空」可能是别的原因
    let ok = apple.top("cn", 5).await.expect("榜单失败");
    println!("对照：正常客户端返回 {} 条", ok.len());
}
