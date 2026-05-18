//! CORS 中间件——仅放行 localhost / 127.0.0.1 origin。
//!
//! Spec：`openspec/specs/http-data-api/spec.md`
//! `Requirement: HTTP server SHALL layer CORS middleware for localhost origins`。
//!
//! 安全模型：与 `start_server` 的 `127.0.0.1` 监听一致，`AllowOrigin::predicate`
//! 显式判断 origin 是否匹配 `^https?://(localhost|127\.0\.0\.1)(:\d+)?$`，
//! 同源请求（如 server 与 webview 同 origin）不触发 CORS 检查、行为不变。

use axum::http::{HeaderName, HeaderValue, Method, header, request::Parts};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// 构建只放行 localhost 来源的 `CorsLayer`。
///
/// 允许的 origin 形态：
/// - `http://localhost`、`http://localhost:<port>`
/// - `https://localhost`、`https://localhost:<port>`
/// - `http://127.0.0.1`、`http://127.0.0.1:<port>`
/// - `https://127.0.0.1`、`https://127.0.0.1:<port>`
///
/// 任何其它 origin（包括 `https://localhost.evil.com`）都不会被 echo
/// 到 `Access-Control-Allow-Origin`，浏览器自然拦截跨源 JS 读响应。
pub fn localhost_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(is_localhost_origin))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([HeaderName::from_static("content-type"), header::ACCEPT])
}

/// 判断 `Origin` header 是否为允许的 localhost 来源。
///
/// Predicate signature: `Fn(&HeaderValue, &Parts) -> bool`，由
/// `tower_http::cors::AllowOrigin::predicate` 调用。
fn is_localhost_origin(origin: &HeaderValue, _parts: &Parts) -> bool {
    let Ok(s) = origin.to_str() else {
        return false;
    };
    is_allowed_origin_str(s)
}

/// 纯字符串判定（便于单测）：origin 形如 `<scheme>://<host>[:<port>]`，
/// 只接受 `http` / `https` 协议 + `localhost` / `127.0.0.1` 主机 + 可选数字端口。
fn is_allowed_origin_str(origin: &str) -> bool {
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    // host[:port] —— 不能含路径或 fragment（合法 Origin header 不会带）。
    if rest.contains('/') || rest.contains('?') || rest.contains('#') {
        return false;
    }
    let (host, port_part) = match rest.find(':') {
        Some(idx) => (&rest[..idx], Some(&rest[idx + 1..])),
        None => (rest, None),
    };
    if host != "localhost" && host != "127.0.0.1" {
        return false;
    }
    match port_part {
        None => true,
        Some(p) => !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_http_localhost_no_port() {
        assert!(is_allowed_origin_str("http://localhost"));
    }

    #[test]
    fn allows_http_localhost_with_port() {
        assert!(is_allowed_origin_str("http://localhost:3456"));
    }

    #[test]
    fn allows_https_127_0_0_1() {
        assert!(is_allowed_origin_str("https://127.0.0.1:8080"));
    }

    #[test]
    fn rejects_localhost_lookalike_subdomain() {
        // 历史绕过尝试：把 localhost 拼到 evil.com 子域上骗 startsWith / contains 类匹配。
        assert!(!is_allowed_origin_str("https://localhost.evil.com"));
        assert!(!is_allowed_origin_str("http://evil.com.localhost.attacker"));
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert!(!is_allowed_origin_str("ftp://localhost"));
        assert!(!is_allowed_origin_str("file://localhost"));
    }

    #[test]
    fn rejects_lan_ips_and_other_hosts() {
        assert!(!is_allowed_origin_str("http://192.168.1.5:3456"));
        assert!(!is_allowed_origin_str("http://example.com"));
        assert!(!is_allowed_origin_str("http://0.0.0.0:3456"));
    }

    #[test]
    fn rejects_path_or_query_in_origin() {
        // 合法 Origin header 不带路径，但保险起见显式拒绝。
        assert!(!is_allowed_origin_str("http://localhost/api"));
        assert!(!is_allowed_origin_str("http://localhost:80?x=1"));
    }

    #[test]
    fn rejects_non_numeric_port() {
        assert!(!is_allowed_origin_str("http://localhost:abc"));
        assert!(!is_allowed_origin_str("http://localhost:"));
    }
}
