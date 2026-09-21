use std::net::{IpAddr, ToSocketAddrs};

use url::Url;

use super::super::error::SkillsError;

/// 解压 / 下载资源限制（原 `LIMITS`）。
pub const MAX_PACKAGE_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_EXTRACTED_BYTES: u64 = 200 * 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_FILE_COUNT: usize = 2000;
pub const MAX_DEPTH: usize = 20;
pub const MAX_COMPRESSION_RATIO: u64 = 100;

/// 可信来源域名白名单（DNS 层校验直接放行）。
const TRUSTED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "codeload.github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "skills.sh",
    "www.skills.sh",
];

/// 判断单个 IP 是否属于环回 / 私网 / 链路本地 / 组播 / 保留段。
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => {
            let octets = address.octets();
            let (a, b) = (octets[0], octets[1]);
            a == 0
                || a == 10
                || a == 127
                || (a == 100 && (64..=127).contains(&b))
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 192 && b == 0)
                || (a == 198 && (b == 18 || b == 19))
                || (a == 169 && b == 254)
                || a >= 224
        }
        IpAddr::V6(address) => {
            // 内嵌 IPv4 的映射地址递归判断。
            if let Some(mapped) = address.to_ipv4_mapped() {
                return is_private_ip(IpAddr::V4(mapped));
            }
            let segments = address.segments();
            address.is_unspecified()
                || address.is_loopback()
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xff00) == 0xff00
        }
    }
}

/// 字符串层拦截指向本地 / 私网 / 链路本地等不可信主机的 URL（SSRF 第一道）。
pub fn assert_public_host(raw: &str) -> Result<(), SkillsError> {
    let url = Url::parse(raw).map_err(|_| SkillsError::bad_request(format!("非法 URL：{raw}")))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(SkillsError::bad_request(format!(
            "仅支持 http(s) 协议：{}",
            url.scheme()
        )));
    }
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_matches(['[', ']'])
        .to_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return Err(SkillsError::bad_request(format!("禁止访问本地地址：{host}")));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err(SkillsError::bad_request(format!("禁止访问私网地址：{host}")));
        }
    }
    Ok(())
}

/// DNS 层校验（SSRF 第二道）：解析主机全部地址，任一命中私网段即拒绝。
pub async fn assert_public_host_resolved(raw: &str) -> Result<(), SkillsError> {
    assert_public_host(raw)?;
    let host = Url::parse(raw)
        .map_err(|_| SkillsError::bad_request(format!("非法 URL：{raw}")))?
        .host_str()
        .unwrap_or_default()
        .trim_matches(['[', ']'])
        .to_lowercase();
    if host.parse::<IpAddr>().is_ok() || TRUSTED_HOSTS.contains(&host.as_str()) {
        return Ok(());
    }
    let host_for_task = host.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let addresses = (host_for_task.as_str(), 0u16)
            .to_socket_addrs()
            .map_err(|_| SkillsError::bad_request(format!("无法解析主机：{host_for_task}")))?;
        let mut resolved = 0;
        for address in addresses {
            resolved += 1;
            if is_private_ip(address.ip()) {
                return Err(SkillsError::bad_request(format!(
                    "禁止访问解析到私网地址的主机：{host_for_task} → {}",
                    address.ip()
                )));
            }
        }
        if resolved == 0 {
            return Err(SkillsError::bad_request(format!(
                "无法解析主机：{host_for_task}"
            )));
        }
        Ok(())
    })
    .await
    .map_err(|error| SkillsError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_private_addresses() {
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.5".parse().unwrap()));
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("169.254.0.1".parse().unwrap()));
        assert!(is_private_ip("::1".parse().unwrap()));
        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("140.82.112.3".parse().unwrap()));
    }

    #[test]
    fn rejects_local_urls() {
        assert!(assert_public_host("http://localhost/x").is_err());
        assert!(assert_public_host("http://127.0.0.1/x").is_err());
        assert!(assert_public_host("file:///etc/passwd").is_err());
        assert!(assert_public_host("https://github.com/a/b").is_ok());
    }
}
