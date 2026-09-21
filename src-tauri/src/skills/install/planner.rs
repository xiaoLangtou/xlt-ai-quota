use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::inspector::{hash_dir, inspect_staging_content};
use super::records::{upsert_install_record, SourceMetadata, UpsertRecordInput};
use super::staging::{get_staging, remove_staging, set_staging_status, staging_content_dir, StagingRegistry};
use super::transaction::{install_skill_to_target, InstallTargetOptions, TargetInstallResult};
use super::super::config::{find_target_root, SkillsPaths};
use super::super::error::SkillsError;
use super::super::paths::safe_join;
use super::super::validation::ValidationIssue;

/// 计划存活时长（同时受 staging 过期约束）。
const PLAN_TTL_SECS: i64 = 15 * 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSkillTarget {
    pub root_id: String,
    pub final_name: String,
    pub conflict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_hash: Option<String>,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSkill {
    pub skill_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub content_hash: String,
    pub size_bytes: u64,
    pub targets: Vec<PlannedSkillTarget>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTarget {
    pub root_id: String,
    pub label: String,
    pub path: String,
    pub writable: bool,
    pub needs_create: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub id: String,
    pub staging_id: String,
    pub created_at: String,
    pub expires_at: String,
    pub source: SourceMetadata,
    pub skills: Vec<PlannedSkill>,
    pub targets: Vec<PlannedTarget>,
    pub issues: Vec<ValidationIssue>,
    pub confirmation_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstallPlanRequest {
    pub staging_id: String,
    pub skill_ids: Vec<String>,
    pub target_root_ids: Vec<String>,
    pub on_conflict: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPlanRequest {
    pub confirmation_hash: String,
    #[serde(default)]
    pub acknowledged_issue_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallOutcome {
    pub skill_id: String,
    pub name: String,
    pub targets: Vec<TargetInstallResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPlanResponse {
    pub plan_id: String,
    pub outcomes: Vec<SkillInstallOutcome>,
}

pub struct PlanEntry {
    pub plan: InstallPlan,
    /// skillId -> staging 内 skill 绝对目录
    pub skill_dirs: HashMap<String, PathBuf>,
}

pub type PlanRegistry = Mutex<HashMap<String, PlanEntry>>;

fn read_existing(root_path: &std::path::Path, name: &str) -> Option<(Option<String>, String)> {
    let dir = root_path.join(name);
    if !dir.exists() {
        return None;
    }
    let version = fs::read_to_string(dir.join("SKILL.md"))
        .ok()
        .and_then(|text| super::super::validation::parse_skill_md(&text).ok())
        .and_then(|parsed| {
            parsed
                .frontmatter
                .as_object()
                .and_then(|map| map.get("version"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        });
    Some((version, hash_dir(&dir)))
}

/// 生成安装计划（重新检查 staging 内容，决策全部固化进计划）。
pub fn create_install_plan(
    paths: &SkillsPaths,
    staging_registry: &StagingRegistry,
    plan_registry: &PlanRegistry,
    request: &CreateInstallPlanRequest,
) -> Result<InstallPlan, SkillsError> {
    let staging = get_staging(paths, staging_registry, &request.staging_id)?;
    if staging.status != "ready" {
        return Err(SkillsError::conflict(format!(
            "staging 当前状态不可生成计划：{}",
            staging.status
        )));
    }
    if request.skill_ids.is_empty() {
        return Err(SkillsError::bad_request("请至少选择一个 Skill"));
    }
    if request.target_root_ids.is_empty() {
        return Err(SkillsError::bad_request("请至少选择一个目标目录"));
    }

    let content_dir = staging_content_dir(paths, &staging.id);
    let search_root = match staging.search_subpath.as_ref() {
        Some(subpath) => Some(safe_join(&content_dir, subpath)?),
        None => None,
    };
    let candidates = inspect_staging_content(&content_dir, search_root.as_deref())?;
    let by_id: HashMap<String, _> = candidates
        .iter()
        .map(|skill| (skill.id.clone(), skill))
        .collect();

    let mut roots = Vec::new();
    for id in &request.target_root_ids {
        let root = find_target_root(paths, id)
            .ok_or_else(|| SkillsError::not_found(format!("根目录不存在：{id}")))?;
        if !root.writable {
            return Err(SkillsError::forbidden(format!("该根目录不可写：{}", root.label)));
        }
        roots.push(root);
    }

    let mut issues: Vec<ValidationIssue> = Vec::new();
    let mut used_names: HashMap<String, HashSet<String>> = HashMap::new();
    let mut skills: Vec<PlannedSkill> = Vec::new();
    for skill_id in &request.skill_ids {
        let skill = by_id
            .get(skill_id)
            .ok_or_else(|| SkillsError::not_found(format!("候选 Skill 不存在：{skill_id}")))?;
        if !skill.installable {
            return Err(SkillsError::new(
                422,
                format!("Skill「{}」存在 error 级问题，无法安装", skill.name),
            ));
        }
        for issue in &skill.issues {
            if issue.level == super::super::validation::ValidationLevel::Warning
                && !issues.iter().any(|existing| existing.code == issue.code)
            {
                issues.push(ValidationIssue {
                    message: format!("{}：{}", skill.name, issue.message),
                    ..issue.clone()
                });
            }
        }
        let mut targets = Vec::new();
        for root in &roots {
            let taken = used_names.entry(root.id.clone()).or_default();
            let conflict = PathBuf::from(&root.path).join(&skill.name).exists() || taken.contains(&skill.name);
            let mut final_name = skill.name.clone();
            let action = if conflict {
                match request.on_conflict.as_str() {
                    "overwrite" => "overwrite",
                    "skip" => "skip",
                    _ => {
                        let mut index = 2;
                        while PathBuf::from(&root.path)
                            .join(format!("{}-{index}", skill.name))
                            .exists()
                            || taken.contains(&format!("{}-{index}", skill.name))
                        {
                            index += 1;
                        }
                        final_name = format!("{}-{index}", skill.name);
                        "keep-both"
                    }
                }
            } else {
                "install"
            };
            if action != "skip" {
                taken.insert(final_name.clone());
            }
            let existing = if conflict {
                read_existing(std::path::Path::new(&root.path), &skill.name)
            } else {
                None
            };
            targets.push(PlannedSkillTarget {
                root_id: root.id.clone(),
                final_name,
                conflict,
                existing_version: existing.as_ref().and_then(|(version, _)| version.clone()),
                existing_hash: existing.map(|(_, hash)| hash),
                action: action.to_owned(),
            });
        }
        skills.push(PlannedSkill {
            skill_id: skill.id.clone(),
            name: skill.name.clone(),
            version: skill.version.clone(),
            content_hash: skill.content_hash.clone(),
            size_bytes: skill.size_bytes,
            targets,
        });
    }

    let targets: Vec<PlannedTarget> = roots
        .iter()
        .map(|root| PlannedTarget {
            root_id: root.id.clone(),
            label: root.label.clone(),
            path: root.path.clone(),
            writable: root.writable,
            needs_create: !PathBuf::from(&root.path).exists(),
        })
        .collect();

    let created = chrono::Utc::now();
    let staging_expires = chrono::DateTime::parse_from_rfc3339(&staging.expires_at)
        .map(|value| value.with_timezone(&chrono::Utc))
        .unwrap_or(created);
    let plan_expires = (created + chrono::Duration::seconds(PLAN_TTL_SECS)).min(staging_expires);
    let confirmation_seed = serde_json::json!({
        "stagingId": staging.id,
        "onConflict": request.on_conflict,
        "skills": skills,
        "targets": targets,
    });
    let confirmation_hash = {
        let digest = Sha1::digest(confirmation_seed.to_string().as_bytes());
        format!("{digest:x}").chars().take(16).collect::<String>()
    };

    let plan = InstallPlan {
        id: uuid::Uuid::new_v4().to_string(),
        staging_id: staging.id.clone(),
        created_at: created.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        expires_at: plan_expires.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        source: staging.source.clone(),
        skills,
        targets,
        issues,
        confirmation_hash,
    };

    let skill_dirs = request
        .skill_ids
        .iter()
        .map(|skill_id| {
            let skill = by_id.get(skill_id).expect("candidate");
            let dir = if skill.relative_path.is_empty() {
                content_dir.clone()
            } else {
                safe_join(&content_dir, &skill.relative_path).expect("safe subpath")
            };
            (skill_id.clone(), dir)
        })
        .collect();

    if let Ok(mut plans) = plan_registry.lock() {
        plans.insert(plan.id.clone(), PlanEntry { plan: plan.clone(), skill_dirs });
    }
    Ok(plan)
}

/// 查找计划；不存在或已过期时报错。
pub fn get_install_plan(plan_registry: &PlanRegistry, id: &str) -> Result<InstallPlan, SkillsError> {
    let plans = plan_registry
        .lock()
        .map_err(|error| SkillsError::internal(error.to_string()))?;
    let entry = plans
        .get(id)
        .ok_or_else(|| SkillsError::not_found("安装计划不存在或已过期"))?;
    let expires = chrono::DateTime::parse_from_rfc3339(&entry.plan.expires_at)
        .map(|value| value.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());
    if chrono::Utc::now() > expires {
        drop(plans);
        if let Ok(mut guard) = plan_registry.lock() {
            guard.remove(id);
        }
        return Err(SkillsError::not_found("安装计划已过期，请重新生成"));
    }
    Ok(entry.plan.clone())
}

/// 取消计划（不动 staging，可重新生成）。
pub fn cancel_install_plan(plan_registry: &PlanRegistry, id: &str) -> bool {
    plan_registry
        .lock()
        .map(|mut plans| plans.remove(id).is_some())
        .unwrap_or(false)
}

/// 提交计划：逐 Skill 逐目标事务安装。
pub fn commit_install_plan(
    paths: &SkillsPaths,
    staging_registry: &StagingRegistry,
    plan_registry: &PlanRegistry,
    plan_id: &str,
    request: &CommitPlanRequest,
) -> Result<CommitPlanResponse, SkillsError> {
    let (plan, skill_dirs) = {
        let plans = plan_registry
            .lock()
            .map_err(|error| SkillsError::internal(error.to_string()))?;
        let entry = plans
            .get(plan_id)
            .ok_or_else(|| SkillsError::not_found("安装计划不存在或已过期"))?;
        (entry.plan.clone(), entry.skill_dirs.clone())
    };
    if request.confirmation_hash != plan.confirmation_hash {
        return Err(SkillsError::conflict("计划内容已变化（确认指纹不匹配），请重新生成计划"));
    }
    let acknowledged: HashSet<&String> = request.acknowledged_issue_codes.iter().collect();
    let pending: Vec<&ValidationIssue> = plan
        .issues
        .iter()
        .filter(|issue| {
            issue.level == super::super::validation::ValidationLevel::Warning
                && !acknowledged.contains(&issue.code)
        })
        .collect();
    if !pending.is_empty() {
        let codes: Vec<&str> = pending.iter().map(|issue| issue.code.as_str()).collect();
        return Err(SkillsError::bad_request(format!(
            "以下风险提示需确认后才能安装：{}",
            codes.join("、")
        )));
    }

    let staging = get_staging(paths, staging_registry, &plan.staging_id)?;
    set_staging_status(paths, staging_registry, &staging.id, "committing")?;

    let mut outcomes = Vec::new();
    for skill in &plan.skills {
        let skill_dir = skill_dirs.get(&skill.skill_id).cloned();
        let mut target_results = Vec::new();
        for target in &skill.targets {
            if target.action == "skip" {
                target_results.push(TargetInstallResult {
                    root_id: target.root_id.clone(),
                    status: "skipped".to_owned(),
                    final_name: Some(target.final_name.clone()),
                    dir_path: None,
                    backup_id: None,
                    error: None,
                });
                continue;
            }
            let Some(root) = find_target_root(paths, &target.root_id) else {
                target_results.push(TargetInstallResult {
                    root_id: target.root_id.clone(),
                    status: "failed".to_owned(),
                    final_name: None,
                    dir_path: None,
                    backup_id: None,
                    error: Some(super::transaction::InstallErrorInfo {
                        code: "root_not_found".to_owned(),
                        message: format!("根目录不存在：{}", target.root_id),
                        stage: "preflight".to_owned(),
                    }),
                });
                continue;
            };
            let Some(skill_dir) = skill_dir.clone() else {
                target_results.push(TargetInstallResult {
                    root_id: target.root_id.clone(),
                    status: "failed".to_owned(),
                    final_name: None,
                    dir_path: None,
                    backup_id: None,
                    error: Some(super::transaction::InstallErrorInfo {
                        code: "staging_missing".to_owned(),
                        message: "staging 内容已丢失，请重新准备来源".to_owned(),
                        stage: "preflight".to_owned(),
                    }),
                });
                continue;
            };
            if !skill_dir.exists() {
                target_results.push(TargetInstallResult {
                    root_id: target.root_id.clone(),
                    status: "failed".to_owned(),
                    final_name: None,
                    dir_path: None,
                    backup_id: None,
                    error: Some(super::transaction::InstallErrorInfo {
                        code: "staging_missing".to_owned(),
                        message: "staging 内容已丢失，请重新准备来源".to_owned(),
                        stage: "preflight".to_owned(),
                    }),
                });
                continue;
            }

            let root_id = root.id.clone();
            let final_name = target.final_name.clone();
            let expected_hash = skill.content_hash.clone();
            let source = staging.source.clone();
            let source_real_path = if staging.source.source_type == "local" {
                Some(staging.source.reference.clone())
            } else {
                None
            };
            let paths_ref: &SkillsPaths = paths;
            target_results.push(install_skill_to_target(InstallTargetOptions {
                skill_dir,
                expected_hash: expected_hash.clone(),
                root,
                final_name: final_name.clone(),
                source_real_path,
                keep_backup: false,
                record: Some(Box::new(move |dir_path, _backup_id| {
                    upsert_install_record(
                        paths_ref,
                        UpsertRecordInput {
                            root_id: root_id.clone(),
                            name: final_name.clone(),
                            dir_path: dir_path.to_string_lossy().to_string(),
                            source: source.clone(),
                            installed_hash: expected_hash.clone(),
                        },
                    )
                    .map(|_| ())
                })),
            }));
        }
        outcomes.push(SkillInstallOutcome {
            skill_id: skill.skill_id.clone(),
            name: skill.name.clone(),
            targets: target_results,
        });
    }

    if let Ok(mut plans) = plan_registry.lock() {
        plans.remove(plan_id);
    }
    let any_failed = outcomes
        .iter()
        .any(|outcome| outcome.targets.iter().any(|target| target.status == "failed"));
    if any_failed {
        let _ = set_staging_status(paths, staging_registry, &staging.id, "ready");
    } else {
        remove_staging(paths, staging_registry, &staging.id);
    }
    Ok(CommitPlanResponse {
        plan_id: plan_id.to_owned(),
        outcomes,
    })
}
