use std::collections::BTreeMap;
use std::fs;
use std::sync::OnceLock;
use std::time::Duration;

use super::paths;
use super::types::{PricingData, PricingHit, PricingQuery, PricingRates, PricingStatus};

const BUILTIN_PRICING: &str = include_str!("pricing.json");

const SUFFIX_PATTERNS: &[&str] = &[
    "-xhigh-fast",
    "-high-fast",
    "-medium-fast",
    "-low-fast",
    "-xhigh",
    "-high",
    "-medium",
    "-low",
    "-fast",
];

fn builtin() -> &'static PricingData {
    static CELL: OnceLock<PricingData> = OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str::<PricingData>(BUILTIN_PRICING).unwrap_or_default())
}

fn overlay() -> Option<PricingData> {
    let content = fs::read_to_string(paths::pricing_overlay_path()).ok()?;
    serde_json::from_str::<PricingData>(&content).ok()
}

/// 内置底表 + 远程覆盖层；覆盖层同名字段优先。
pub fn merged() -> PricingData {
    let base = builtin();
    let mut data = base.clone();
    if let Some(remote) = overlay() {
        for (key, rates) in remote.exact {
            data.exact.insert(key, rates);
        }
        for (key, value) in remote.alias {
            data.alias.insert(key, value);
        }
        for (source, aliases) in remote.source_alias {
            let entry = data.source_alias.entry(source).or_default();
            for (key, value) in aliases {
                entry.insert(key, value);
            }
        }
        if !remote.fuzzy.is_empty() {
            let mut fuzzy = remote.fuzzy;
            fuzzy.extend(data.fuzzy);
            data.fuzzy = fuzzy;
        }
        if remote.default.is_some() {
            data.default = remote.default;
        }
    }
    data
}

fn default_rates() -> PricingRates {
    PricingRates {
        input: 1.0,
        output: 5.0,
        cache_read: 0.1,
        cache_write: 1.25,
    }
}

fn strip_reasoning_suffix(model: &str) -> String {
    for pattern in SUFFIX_PATTERNS {
        if model.ends_with(pattern) {
            return model[..model.len() - pattern.len()].to_owned();
        }
    }
    model.to_owned()
}

fn normalize_claude_model(model: &str) -> String {
    let base = model.rsplit('/').next().unwrap_or(model);
    let mut normalized = base
        .trim()
        .to_lowercase()
        .replace(['(', ')'], " ")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '.' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while normalized.starts_with('-') {
        normalized.remove(0);
    }
    while normalized.ends_with('-') {
        normalized.pop();
    }
    while normalized.contains("--") {
        normalized = normalized.replace("--", "-");
    }

    // claude-sonnet-4.5 → claude-sonnet-4-5
    if normalized.starts_with("claude-sonnet-")
        || normalized.starts_with("claude-opus-")
        || normalized.starts_with("claude-haiku-")
    {
        if let Some((head, tail)) = normalized.split_once('.') {
            normalized = format!("{head}-{tail}");
        }
    }
    normalized
}

fn normalize_for_source(model: &str, source: Option<&str>) -> String {
    let base = strip_reasoning_suffix(model.trim());
    match source {
        Some("claude") => normalize_claude_model(&base),
        _ => base,
    }
}

fn exact_lookup(table: &BTreeMap<String, PricingRates>, key: &str) -> Option<PricingRates> {
    if let Some(rates) = table.get(key) {
        return Some(rates.clone());
    }
    let lower = key.to_lowercase();
    table
        .iter()
        .find(|(name, _)| name.to_lowercase() == lower)
        .map(|(_, rates)| rates.clone())
}

fn contained_lookup(table: &BTreeMap<String, PricingRates>, key: &str) -> Option<PricingRates> {
    let lower = key.to_lowercase();
    let mut keys: Vec<&String> = table.keys().collect();
    keys.sort_by_key(|value| std::cmp::Reverse(value.len()));
    keys.into_iter()
        .find(|name| lower.contains(&name.to_lowercase()))
        .and_then(|name| table.get(name).cloned())
}

fn dot_restored(model: &str) -> Option<String> {
    let lower = model.to_lowercase();
    let mut restored = String::with_capacity(lower.len());
    let chars: Vec<char> = lower.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let character = chars[index];
        if character == '-' && index > 0 && index + 1 < chars.len() {
            let prev_digit = chars[index - 1].is_ascii_digit();
            let next_digit = chars[index + 1].is_ascii_digit();
            if prev_digit && next_digit {
                restored.push('.');
                index += 1;
                continue;
            }
        }
        restored.push(character);
        index += 1;
    }
    (restored != lower).then_some(restored)
}

fn resolve_ref(
    data: &PricingData,
    reference: &str,
) -> Option<(PricingRates, &'static str)> {
    if let Some(rates) = exact_lookup(&data.exact, reference) {
        return Some((rates, "alias"));
    }
    if let Some(rates) = contained_lookup(&data.exact, reference) {
        return Some((rates, "alias"));
    }
    None
}

pub fn lookup(model: &str, source: Option<&str>) -> PricingHit {
    let data = merged();
    let normalized = normalize_for_source(model, source);
    let default = data.default.clone().unwrap_or_else(default_rates);

    let mut candidates = vec![normalized.clone()];
    if let Some(restored) = dot_restored(&normalized) {
        candidates.push(restored);
    }

    for candidate in &candidates {
        if let Some(rates) = exact_lookup(&data.exact, candidate) {
            return PricingHit {
                model: model.to_owned(),
                rates,
                matched: true,
                via: "exact".to_owned(),
            };
        }
    }

    if let Some(source) = source {
        if let Some(aliases) = data.source_alias.get(source) {
            for candidate in &candidates {
                if let Some(reference) = aliases.get(candidate) {
                    if let Some((rates, via)) = resolve_ref(&data, reference) {
                        return PricingHit {
                            model: model.to_owned(),
                            rates,
                            matched: true,
                            via: format!("sourceAlias:{via}"),
                        };
                    }
                }
            }
        }
    }

    for candidate in &candidates {
        if let Some(reference) = data.alias.get(candidate) {
            if let Some((rates, via)) = resolve_ref(&data, reference) {
                return PricingHit {
                    model: model.to_owned(),
                    rates,
                    matched: true,
                    via: format!("alias:{via}"),
                };
            }
        }
    }

    for candidate in &candidates {
        for rule in &data.fuzzy {
            if candidate.contains(&rule.r#match.to_lowercase()) {
                if let Some((rates, via)) = resolve_ref(&data, &rule.r#ref) {
                    return PricingHit {
                        model: model.to_owned(),
                        rates,
                        matched: true,
                        via: format!("fuzzy:{via}"),
                    };
                }
            }
        }
    }

    for candidate in &candidates {
        if let Some(rates) = contained_lookup(&data.exact, candidate) {
            return PricingHit {
                model: model.to_owned(),
                rates,
                matched: true,
                via: "contained".to_owned(),
            };
        }
    }

    PricingHit {
        model: model.to_owned(),
        rates: default,
        matched: false,
        via: "default".to_owned(),
    }
}

pub fn lookup_many(queries: &[PricingQuery]) -> Vec<PricingHit> {
    queries
        .iter()
        .map(|query| lookup(&query.model, query.source.as_deref()))
        .collect()
}
pub fn status() -> PricingStatus {
    let data = merged();
    let overlay_path = paths::pricing_overlay_path();
    let updated_at = fs::metadata(&overlay_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
    PricingStatus {
        has_overlay: overlay_path.exists(),
        url: Some("https://api.juejin.cn/aiusage_api/functions/tud-pricing".to_owned()),
        updated_at,
        exact_count: data.exact.len(),
        alias_count: data.alias.len(),
        fuzzy_count: data.fuzzy.len(),
    }
}

/// 拉取远程覆盖层并落盘。失败时保留上一版，不阻断应用启动。
pub fn refresh_overlay(url: &str) -> Result<PricingStatus, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("xlt-workbench/0.1 (+usage pricing overlay)")
        .build()
        .map_err(|error| format!("创建定价请求失败: {error}"))?;
    let response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("拉取定价覆盖层失败: {error}"))?;
    let body = response
        .text()
        .map_err(|error| format!("读取定价覆盖层失败: {error}"))?;
    let parsed: PricingData =
        serde_json::from_str(&body).map_err(|error| format!("定价覆盖层格式非法: {error}"))?;
    if parsed.exact.is_empty() && parsed.alias.is_empty() && parsed.fuzzy.is_empty() {
        return Err("定价覆盖层为空，已保留本地版本".to_owned());
    }
    write_overlay(&body)?;
    Ok(status())
}

fn write_overlay(body: &str) -> Result<(), String> {
    let path = paths::pricing_overlay_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, body).map_err(|error| format!("写入定价覆盖层失败: {error}"))?;
    fs::rename(&temp, &path).map_err(|error| format!("替换定价覆盖层失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_alias_fuzzy_and_default_are_resolved() {
        let exact = lookup("anthropic/claude-opus-4-8", None);
        assert!(exact.matched);
        assert_eq!(exact.rates.input, 5.0);
        assert_eq!(exact.rates.cache_write, 6.25);

        let gemini = lookup("gemini-2.5-flash", None);
        assert!(gemini.matched);
        assert_eq!(gemini.rates.output, 2.5);

        let unknown = lookup("totally-unknown-model-xyz", None);
        assert!(!unknown.matched);
        assert_eq!(unknown.via, "default");
        assert_eq!(unknown.rates.input, 1.0);
    }

    #[test]
    fn claude_source_normalization_handles_dotted_versions() {
        let hit = lookup("claude-sonnet-4.5", Some("claude"));
        assert!(hit.matched);
        assert_eq!(hit.rates.input, 3.0);
        assert_eq!(hit.rates.output, 15.0);
    }
}
