//! 日历、待办与文档拆解领域：本地优先的计划执行入口。
//! 数据保存在本机 SQLite，AI 调用复用 Git 报告的供应商配置（由前端读取后传入）。

pub mod commands;
pub mod decomposition;
pub mod document;
pub mod store;
pub mod types;
pub mod zentao;

pub mod zentao_summary;

mod zentao_materials;
