//! jianying-cli library: plan schema, capability catalogs, draft assembly,
//! probing, store handling, template mode, SRT and proxy rendering.
pub mod capabilities;
pub mod caption_ops;
pub mod catalogs;
pub mod compile_ops;
pub mod control_catalog;
pub mod domain_compat;
pub mod draft;
pub mod error_contract;
pub mod fixture_ops;
pub mod interchange;
pub mod job_runner;
pub mod mcp_server;
pub mod media_analysis;
pub mod media_ops;
pub mod plan;
pub mod probe;
pub mod project_ops;
pub mod render;
pub mod srt;
pub mod store;
pub mod template;
pub mod tim;
pub mod timeline_ops;

/// CLI 结构化成功和失败响应。
pub use jianying_cli_contract as cli_contract;
/// 统一、与草稿 wire 格式和 CLI 解耦的领域模型。
pub use jianying_domain as domain;
/// 原始草稿无损编辑 envelope。
pub use jianying_draft as lossless_draft;
/// 可恢复作业状态机。
pub use jianying_jobs as jobs;
/// MCP 稳定工具注册表。
pub use jianying_mcp as mcp;
/// 媒体探测后的领域对象。
pub use jianying_media as media;
/// 运行时能力档案。
pub use jianying_runtime as runtime;
/// 跨宿主 Job v2 协议。
pub use jianying_schema as schema;
/// 草稿存储安全路径模型。
pub use jianying_store as store_model;
