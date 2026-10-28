//! OPML 往返测试（不联网）：解析脏 OPML、导出再解析回来。

use dear_universe_podcast_lib::core::{db, opml};

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>My Feeds</title></head>
  <body>
    <outline text="Life Kit" title="Life Kit" type="rss" xmlUrl="https://feeds.npr.org/510338/podcast.xml"/>
    <outline text="Planet Money" type="rss" xmlUrl="https://feeds.npr.org/510289/podcast.xml"/>
    <!-- 同一个地址写两遍，真实导出的 OPML 里很常见 -->
    <outline text="Life Kit (dup)" type="rss" xmlUrl="https://feeds.npr.org/510338/podcast.xml"/>
    <!-- 没有 xmlUrl 的分组节点，应当被忽略 -->
    <outline text="分组"/>
    <outline text="含 &amp; 与引号" type="rss" xmlUrl="https://a.example/f.xml?x=1&amp;y=2"/>
  </body>
</opml>
"#;

#[test]
fn parses_outlines_and_dedupes() {
    let feeds = opml::read_feeds(SAMPLE).expect("解析失败");

    // 两条有效 + 一条带转义参数的，重复的那个被去掉
    assert_eq!(feeds.len(), 3, "解析结果：{feeds:?}");

    let urls: Vec<&str> = feeds.iter().map(|(u, _)| u.as_str()).collect();
    assert!(urls.contains(&"https://feeds.npr.org/510338/podcast.xml"));
    assert!(urls.contains(&"https://feeds.npr.org/510289/podcast.xml"));

    // 实体必须还原，否则订不到正确的地址
    let escaped = feeds
        .iter()
        .find(|(u, _)| u.starts_with("https://a.example"))
        .expect("带转义的源没解析出来");
    assert_eq!(escaped.0, "https://a.example/f.xml?x=1&y=2");
}

#[test]
fn titles_fall_back_to_url() {
    // 只有 xmlUrl 没有 text 的写法也要能解析，标题退化成地址
    let bare = r#"<opml><body><outline xmlUrl="https://b.example/f.xml"/></body></opml>"#;
    let feeds = opml::read_feeds(bare).expect("解析失败");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].1, "https://b.example/f.xml");
}

#[test]
fn broken_opml_does_not_error() {
    // 截断的文件：能解析出多少算多少，关键是不能抛错让整个导入失败
    let broken = r#"<opml><body><outline xmlUrl="https://c.example/f.xml""#;
    let feeds = opml::read_feeds(broken).expect("不该报错");
    // 这条属性是完整的，所以解析得出来；真正残缺的会被丢掉
    assert_eq!(feeds.len(), 1);

    let garbage = "<opml><body><outline xmlUr";
    assert!(opml::read_feeds(garbage).expect("不该报错").is_empty());
}

#[test]
fn exported_opml_round_trips() {
    let dir = tempfile::tempdir().expect("建临时目录失败");
    let db = db::Database::open_in(dir.path()).expect("开库失败");

    {
        let conn = db.read().expect("拿连接失败");
        let dto = db::podcast::new_dto(
            "https://feeds.npr.org/510338/podcast.xml".to_string(),
            "Life Kit & Friends".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        db::podcast::upsert(&conn, &dto, "rss").expect("入库失败");
    }

    let xml = opml::build(&db).expect("导出失败");
    assert!(xml.contains("<opml"), "导出的不是 OPML：{xml}");

    // 名字里的 & 必须转义，否则别的客户端读到的是坏 XML
    assert!(xml.contains("Life Kit &amp; Friends"));

    let back = opml::read_feeds(&xml).expect("读回失败");
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].0, "https://feeds.npr.org/510338/podcast.xml");
}
