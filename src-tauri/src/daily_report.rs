use chrono::NaiveDate;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug, Deserialize)]
pub struct GitProjectInput {
    path: String,
    name: String,
    branch: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProject {
    path: String,
    name: String,
    branch: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    hash: String,
    message: String,
    date: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProjectCommits {
    project: GitProject,
    commits: Vec<GitCommit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLogInput {
    projects: Vec<GitProjectInput>,
    authors: Vec<String>,
    start_date: String,
    end_date: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerateRequest {
    provider: String,
    model: String,
    api_key: String,
    report_type: String,
    start_date: String,
    end_date: String,
    projects: Vec<GitProjectCommits>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitReportImport {
    projects: Vec<GitProject>,
    authors: Vec<String>,
}

#[tauri::command]
pub fn daily_report_validate_project(path: String) -> Result<GitProject, String> {
    let root = git_output(&path, &["rev-parse", "--show-toplevel"])?;
    let name = root
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "无法读取 Git 仓库名称".to_owned())?
        .to_owned();
    let branch = git_output(&path, &["branch", "--show-current"])?;
    Ok(GitProject {
        path: root,
        name,
        branch: if branch.is_empty() {
            "HEAD（游离）".to_owned()
        } else {
            branch
        },
    })
}

#[tauri::command]
pub fn daily_report_collect_commits(input: GitLogInput) -> Result<Vec<GitProjectCommits>, String> {
    validate_range(&input.start_date, &input.end_date)?;
    input
        .projects
        .iter()
        .map(|project| {
            collect_project_commits(project, &input.authors, &input.start_date, &input.end_date)
        })
        .collect()
}

#[tauri::command]
pub fn daily_report_generate(input: AiGenerateRequest) -> Result<String, String> {
    validate_range(&input.start_date, &input.end_date)?;
    if input.api_key.trim().is_empty() {
        return Err("请输入 AI Provider 的 API Key".to_owned());
    }
    if input.model.trim().is_empty() {
        return Err("请选择 AI 模型".to_owned());
    }
    let report_type = match input.report_type.as_str() {
        "daily" => "日报",
        "weekly" => "周报",
        _ => return Err("报告类型无效".to_owned()),
    };
    let prompt = build_report_prompt(
        &input.projects,
        report_type,
        &input.start_date,
        &input.end_date,
    )?;
    let body = serde_json::json!({
        "model": input.model,
        "messages": [
            { "role": "system", "content": "你是专业的技术工作总结助手。仅根据 Git 提交记录，用中文生成准确、简洁的工作总结。" },
            { "role": "user", "content": prompt },
        ],
        "temperature": 0.3,
        "max_tokens": 2000,
    });
    let (host, path) = match input.provider.as_str() {
        "minimax" => ("api.minimax.chat", "/v1/text/chatcompletion_v2"),
        "kimi" => ("api.moonshot.cn", "/v1/chat/completions"),
        "deepseek" => ("api.deepseek.com", "/chat/completions"),
        _ => return Err("不支持的 AI Provider".to_owned()),
    };
    let output = Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg("15")
        .arg("--max-time")
        .arg("60")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("Authorization: Bearer {}", input.api_key.trim()))
        .arg("--data-raw")
        .arg(body.to_string())
        .arg(format!("https://{host}{path}"))
        .output()
        .map_err(|error| format!("无法调用 AI Provider：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "AI Provider 请求失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let response: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("AI Provider 返回的不是 JSON：{error}"))?;
    let content = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            let message = response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .or_else(|| response.get("message").and_then(Value::as_str))
                .unwrap_or("AI Provider 未返回报告内容");
            format!("AI Provider 返回错误：{message}")
        })?;
    let thinking = Regex::new(r"(?s)<think>.*?</think>")
        .map_err(|error| format!("无法清理模型思考内容：{error}"))?;
    let report = thinking.replace_all(content, "").trim().to_owned();
    if report.is_empty() {
        return Err("AI Provider 未返回可复制的报告内容".to_owned());
    }
    Ok(report)
}

#[tauri::command]
pub fn daily_report_import_gitreports() -> Result<GitReportImport, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "无法读取用户主目录".to_owned())?;
    let config_path = home.join(".git-reporter").join("config.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("无法读取 GitReport 配置：{error}"))?;
    let config: Value = serde_json::from_str(&content)
        .map_err(|error| format!("GitReport 配置不是 JSON：{error}"))?;
    let projects = config
        .get("projects")
        .and_then(Value::as_array)
        .ok_or_else(|| "GitReport 配置缺少 projects".to_owned())?
        .iter()
        .map(|item| {
            Ok(GitProject {
                path: required_string(item, "path")?,
                name: required_string(item, "name")?,
                branch: String::new(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let authors = config
        .get("authors")
        .and_then(Value::as_array)
        .ok_or_else(|| "GitReport 配置缺少 authors".to_owned())?
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "GitReport authors 包含非文本内容".to_owned())
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(GitReportImport { projects, authors })
}

fn build_report_prompt(
    projects: &[GitProjectCommits],
    report_type: &str,
    start_date: &str,
    end_date: &str,
) -> Result<String, String> {
    let date_label = if start_date == end_date {
        start_date.to_owned()
    } else {
        format!("{start_date} 至 {end_date}")
    };
    let mut lines = vec![format!(
        "请根据以下 {date_label} 的 Git 提交记录生成{report_type}。"
    )];
    let mut commit_count = 0usize;
    for group in projects {
        if group.commits.is_empty() {
            continue;
        }
        lines.push(String::new());
        lines.push(format!("【{}】", group.project.name));
        for commit in &group.commits {
            lines.push(format!("- [{}] {}", commit.date, commit.message));
            commit_count += 1;
        }
    }
    if commit_count == 0 {
        return Err("所选时间范围没有可用于生成报告的 Git 提交".to_owned());
    }
    lines.push(String::new());
    lines.push("输出要求：".to_owned());
    if report_type == "日报" {
        lines.push("- 直接输出编号列表，不要标题、不要总结段落".to_owned());
        lines.push("- 将 commit message 转为自然中文，去掉 feat、fix、refactor 等前缀".to_owned());
        lines.push("- 合并相近提交，每条不超过 30 字".to_owned());
    } else {
        lines.push("- 使用“本周完成”和“下周计划”两个小标题".to_owned());
        lines
            .push("- 本周完成按编号列表归纳 Git 提交；下周计划根据未完成的工作合理延续".to_owned());
        lines.push("- 语言简洁，避免罗列 commit hash 或技术前缀".to_owned());
    }
    Ok(lines.join("\n"))
}

fn collect_project_commits(
    project: &GitProjectInput,
    authors: &[String],
    start_date: &str,
    end_date: &str,
) -> Result<GitProjectCommits, String> {
    let mut args = vec![
        "-c".to_owned(),
        "core.quotepath=false".to_owned(),
        "-C".to_owned(),
        project.path.clone(),
        "log".to_owned(),
        "--all".to_owned(),
        format!("--after={start_date} 00:00:00"),
        format!("--before={end_date} 23:59:59"),
        "--no-merges".to_owned(),
        "--pretty=format:%h%x09%s%x09%ad".to_owned(),
        "--date=format:%Y-%m-%d %H:%M".to_owned(),
    ];
    for author in authors
        .iter()
        .map(|author| author.trim())
        .filter(|author| !author.is_empty())
    {
        args.push(format!("--author={author}"));
    }
    let output = Command::new("git")
        .args(&args)
        .output()
        .map_err(|error| format!("无法执行 git：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "读取 {} 的提交记录失败：{}",
            project.name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let commits = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.splitn(3, '\t');
            let hash = fields.next().unwrap_or_default().to_owned();
            let message = fields.next().unwrap_or_default().to_owned();
            let date = fields.next().unwrap_or_default().to_owned();
            if hash.is_empty() || message.is_empty() || date.is_empty() {
                return Err(format!("{} 返回了无法解析的 Git 提交记录", project.name));
            }
            Ok(GitCommit {
                hash,
                message,
                date,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GitProjectCommits {
        project: GitProject {
            path: project.path.clone(),
            name: project.name.clone(),
            branch: project.branch.clone(),
        },
        commits,
    })
}

fn git_output(path: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|error| format!("无法执行 git：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "不是有效的 Git 仓库：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn required_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("GitReport 配置缺少 {key}"))
}

fn validate_range(start_date: &str, end_date: &str) -> Result<(), String> {
    let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
        .map_err(|_| "日期格式应为 YYYY-MM-DD".to_owned())?;
    let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
        .map_err(|_| "日期格式应为 YYYY-MM-DD".to_owned())?;
    if start > end {
        return Err("开始日期不能晚于结束日期".to_owned());
    }
    Ok(())
}
