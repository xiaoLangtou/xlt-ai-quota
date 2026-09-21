// 分阶段移植：P0 只落地只读层，部分工具函数与后续阶段模块暂未被调用。
#![allow(dead_code)]

pub mod commands;
pub mod config;
pub mod error;
pub mod install;
pub mod paths;
pub mod remote_detail;
pub mod scan;
pub mod skill_io;
pub mod skillssh;
pub mod state;
pub mod trash;
pub mod validation;

pub use state::SkillsState;
