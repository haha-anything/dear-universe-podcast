//! 命令层：只做参数校验和转发，业务逻辑在 crate::core（AGENTS.md 分层规则）。
//! 所有命令统一返回 Result<T, String>，前端拿到的错误文本可直接展示。

pub mod episode;
pub mod feed;
pub mod legacy;
pub mod progress;
pub mod queue;
pub mod session;
