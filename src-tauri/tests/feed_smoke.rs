//! 联网冒烟测试：真实拉一个 RSS，验证抓取 → 解析 → DTO 的完整链路。
//!
//! 需要联网，所以默认 #[ignore]。手动跑：
//!   cargo test --test feed_smoke -- --ignored --nocapture

use dear_universe_podcast_lib::{core::feed, core::http::HttpClient};

const NPR_HIDDEN_BRAIN: &str = "https://feeds.npr.org/510338/podcast.xml";

#[tokio::test]
#[ignore = "需要联网：cargo test --test feed_smoke -- --ignored --nocapture"]
async fn parses_real_feed() {
    let http = HttpClient::new().expect("构建 HTTP 客户端失败");
    let client = http.inner().clone();

    let feed = feed::fetch_feed(&client, NPR_HIDDEN_BRAIN)
        .await
        .expect("抓取解析失败");

    println!("标题: {}", feed.title);
    println!("作者: {:?}", feed.author);
    println!("封面: {:?}", feed.image_url);
    println!("单集数: {}", feed.episodes.len());

    assert!(!feed.title.is_empty(), "标题不能为空");
    assert!(!feed.episodes.is_empty(), "应该有单集");

    for ep in feed.episodes.iter().take(3) {
        println!(
            "- {} | {}s | {}",
            ep.title,
            ep.duration_sec.map(|d| d.to_string()).unwrap_or_else(|| "?".into()),
            ep.audio_url
        );
    }

    // 每一条都得有音频地址，否则前端点了播不出声
    assert!(
        feed.episodes.iter().all(|e| e.audio_url.starts_with("http")),
        "存在没有音频地址的单集"
    );

    // NPR 的 itunes:duration 是纯秒数，解析出来应该落在合理区间
    let with_duration = feed.episodes.iter().filter(|e| e.duration_sec.is_some()).count();
    assert!(with_duration > 0, "一条时长都没解析出来，itunes:duration 映射有问题");
}
