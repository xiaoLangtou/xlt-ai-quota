// 分阶段实现：部分适配器 / 源工具函数暂未被命令直接调用。
#![allow(dead_code)]

pub mod adapters;
pub mod catalog;
pub mod commands;
pub mod error;
pub mod metadata;
pub mod parse;
pub mod paths;
pub mod plan;
pub mod probe;
pub mod scan;
pub mod state;
pub mod types;
pub mod write;

pub use state::McpState;
