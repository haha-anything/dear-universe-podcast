//! OPML 导入导出。
//!
//! OPML 是播客客户端之间搬订阅列表的通用格式。导出给别的客户端用，
//! 导入则从别的客户端搬过来。
//!
//! 导出只写订阅（podcast 表），不写单集 —— OPML 的格式本来就只装订阅。
//!
//! **没有引入 XML 解析库**：OPML 的正文就是一堆 `<outline ... />`，
//! 手写一个只有几十行的扫描器就够，还能顺带容忍各种脏写法。
//! 多一个依赖就多一份构建期的网络风险（这次就是被 crates 镜像卡住的）。

use std::collections::HashSet;

use crate::{
    core::db::{self, Database},
    error::AppError,
    model::OpmlImportReport,
};

/// 导出：把所有订阅写成 OPML 2.0 文本。
///
/// 返回文本而不是写文件：没有装文件对话框插件，路径得用户手敲，
/// 那体验不如让前端拿这段文本直接触发一次浏览器下载。
pub fn build(db: &Database) -> Result<String, AppError> {
    let podcasts = {
        let conn = db.read()?;
        db::podcast::list(&conn)?
    };

    let mut body = String::with_capacity(podcasts.len() * 200);
    for p in &podcasts {
        let title = xml_escape(&p.title);
        let url = xml_escape(&p.origin);
        // text 属性是给别的客户端显示的，用 custom_title 更符合用户预期
        let text = xml_escape(if p.custom_title.is_empty() {
            &p.title
        } else {
            &p.custom_title
        });

        body.push_str(&format!(
            "    <outline type=\"rss\" text=\"{text}\" title=\"{title}\" xmlUrl=\"{url}\" />\n"
        ));
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <opml version=\"2.0\">\n\
         \x20 <head>\n\
         \x20   <title>大宇宙播客 订阅</title>\n\
         \x20 </head>\n\
         \x20 <body>\n\
         {body}\x20 </body>\n\
         </opml>\n"
    );

    Ok(xml)
}

/// 导入：只读解析，把取到的 feedUrl 交给调用方去订阅。
///
/// 这里**不直接订阅**，因为订阅要联网抓 feed，那是上百次网络请求，
/// 得让前端先看到「要导入 N 个」再决定。
///
/// 吃的是文本内容而不是路径：文件由前端的 `<input type="file">` 读进来，
/// 省掉一个文件对话框插件。
pub fn read_feeds(raw: &str) -> Result<Vec<(String, String)>, AppError> {
    let mut feeds: Vec<(String, String)> = Vec::new();

    // 逐个 `<` 切开，只认 outline 标签，其余一概跳过。
    // 这样 CDATA、注释、编码写错的文件都能撑住。
    for chunk in raw.split('<') {
        // 自闭合标签形如 `outline ... />`，去掉尾部那个 `/` 再切属性
        let tag = chunk
            .split('>')
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches('/')
            .trim();
        if !tag.starts_with("outline") {
            continue;
        }

        let mut url = None;
        let mut title = None;
        for attr in split_attributes(tag) {
            let (key, value) = match attr.split_once('=') {
                Some(pair) => pair,
                None => continue,
            };
            let value = unescape(&value.trim_matches(|c| c == '"' || c == '\'').to_string());

            if key.eq_ignore_ascii_case("xmlUrl") {
                url = Some(value);
            } else if key.eq_ignore_ascii_case("text") || key.eq_ignore_ascii_case("title") {
                title = Some(value);
            }
        }

        if let Some(u) = url.filter(|s| s.starts_with("http")) {
            feeds.push((u.clone(), title.unwrap_or(u)));
        }
    }

    // 同一个 feed 写两遍的 OPML 不少见，按 url 去重
    let mut seen = HashSet::new();
    feeds.retain(|(url, _)| seen.insert(url.clone()));

    Ok(feeds)
}

/// 按空格切属性，但属性值里的空格不算分隔
fn split_attributes(tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for c in tag.chars() {
        match (quote, c) {
            (None, '"' | '\'') => {
                quote = Some(c);
                current.push(c);
            }
            (Some(q), c) if c == q => {
                quote = None;
                current.push(c);
            }
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            (_, c) => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// 把一批 feed 订阅进库。逐条订阅，失败的记录下来继续下一条。
pub async fn import_feeds(
    db: &Database,
    client: &reqwest::Client,
    feeds: &[(String, String)],
) -> Result<OpmlImportReport, AppError> {
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    for (url, title) in feeds {
        let already = {
            let conn = db.read()?;
            db::podcast::exists(&conn, url)?
        };

        if already {
            skipped += 1;
            continue;
        }

        match crate::core::subscription::subscribe(db, client, url).await {
            Ok(_) => imported += 1,
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "OPML 里的源订阅失败");
                errors.push(format!("{title}：{e}"));
            }
        }
    }

    Ok(OpmlImportReport {
        imported,
        skipped,
        errors,
    })
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
