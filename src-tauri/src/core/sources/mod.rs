//! 内容源。
//!
//! # 合规红线（任务书 §6，改这个目录前先读完）
//!
//! 允许的三个端点，**只有这些**：
//!
//! ```text
//! 搜索  GET https://itunes.apple.com/search?term={q}&media=podcast&country=cn&limit=50
//! 详情  GET https://itunes.apple.com/lookup?id={collectionId}&entity=podcast
//! 榜单  GET https://itunes.apple.com/{cn|us}/rss/toppodcasts/limit=100/json
//! ```
//!
//! 硬禁止：
//!   - 爬任何网页（小宇宙、喜马拉雅、Apple 网页版都不行）
//!   - 逆向私有接口、模拟登录
//!   - 镜像 / 转存节目内容
//!   - 把 Apple 返回的节目信息当自有数据长期存储（只存 feedUrl 和必要展示字段）
//!
//! 目录只用来「发现 + 取 feedUrl」，**音频一律从节目自己的 RSS/CDN 拉，不经过 Apple**。
//! Apple 不可达时其余功能必须照常，见 §6.3。

pub mod apple;
