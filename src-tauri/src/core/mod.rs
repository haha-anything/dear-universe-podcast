//! 业务逻辑层。命令层只做参数校验和转发，真正的活在这里（AGENTS.md 分层规则）。

pub mod db;
pub mod download;
pub mod feed;
pub mod http;
pub mod legacy;
pub mod opml;
pub mod sources;
pub mod subscription;
