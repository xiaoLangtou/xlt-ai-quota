//! Shell 环境管理：别名 + 托管环境变量 + 源码视图 + 备份恢复。
//!
//! 模块结构（PRD-开发环境管理后续规划 7.6）：
//! - `types` DTO 与值语义约定
//! - `parser` 行切分、条目解析、结构上下文、区块扫描
//! - `serializer` 字面量引号规则、会话命令生成
//! - `files` 版本哈希、字节范围补丁、安全写入、备份
//! - `plan` 变更计划：preview / commit / cancel
//! - `commands` Tauri 命令边界

mod files;
mod parser;
mod pathmodel;
mod plan;
mod serializer;
mod source;
mod types;

pub mod commands;

#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod source_tests;
#[cfg(test)]
mod tests;
