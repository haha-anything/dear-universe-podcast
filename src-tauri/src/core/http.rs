//! HTTP 客户端。
//!
//! 全进程共用一个 `reqwest::Client`（内部本是 Arc，clone 很便宜）复用连接池，
//! 并统一设置 UA 和超时。
//! 实例通过 Tauri 的 managed state 注入，命令里 `State<'_, HttpClient>` 拿到手。

use std::time::Duration;

use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT},
    Client,
};

use crate::error::AppError;

/// RSS 抓取超时。播客 feed 动辄几 MB，太短会误杀慢源。
const FEED_TIMEOUT: Duration = Duration::from_secs(30);

/// UA 用浏览器的通用串，而不是 `DearUniversePodcast/x.y`。
///
/// 实测：小宇宙的 feed 域名 feed.xyzfm.space 对非浏览器 UA 一律 403，
/// 本机 21 个订阅里有 18 个躺在这个域上，全挂。而各家 RSS 客户端
/// （以及 curl --http1.1 -A "Mozilla/5.0"）都是这么处理的 ——
/// 这是 CDN 的兼容性问题，不是绕过鉴权，这里不涉及任何私有接口。
///
/// 想表明身份的话走 `X-` 头或者 feed 里的 `<generator>`，别堵死自己。
pub(crate) const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
}

impl HttpClient {
    /// 构造时要失败就早失败，不要等到第一个请求才发现 UA 非法
    pub fn new() -> Result<Self, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        // 优先要 XML/RSS，有些站不看 Accept 会返回 HTML 页面
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/rss+xml, application/xml, text/xml, */*"),
        );

        let inner = Client::builder()
            .default_headers(headers)
            .gzip(true)
            .timeout(FEED_TIMEOUT)
            .build()?;

        Ok(Self { inner })
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }
}
