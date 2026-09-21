use std::collections::HashMap;
use std::sync::Mutex;

use super::config::SkillsPaths;
use super::install::catalog::StarsCache;
use super::install::planner::PlanRegistry;
use super::install::staging::StagingRegistry;
use super::scan::ScanFailure;

/// Skills 模块运行态。
///
/// 持有路径配置、最近一次扫描的解析失败项，以及安装流水线的内存态
/// （staging / 安装计划 / star 缓存）。
pub struct SkillsState {
    pub paths: SkillsPaths,
    pub scan_failures: Mutex<Vec<ScanFailure>>,
    pub staging: StagingRegistry,
    pub plans: PlanRegistry,
    pub stars_cache: StarsCache,
}

impl SkillsState {
    pub fn new() -> Self {
        Self {
            paths: SkillsPaths::detect(),
            scan_failures: Mutex::new(Vec::new()),
            staging: Mutex::new(HashMap::new()),
            plans: Mutex::new(HashMap::new()),
            stars_cache: Mutex::new(None),
        }
    }
}

impl Default for SkillsState {
    fn default() -> Self {
        Self::new()
    }
}
