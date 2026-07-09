use regex::Regex;

pub struct Redactor {
    patterns: Vec<Regex>,
    enabled: bool,
}

impl Redactor {
    pub fn new(enabled: bool) -> Self {
        let patterns = if enabled {
            vec![
                Regex::new(r"sk-[a-zA-Z0-9_\-]{20,}").unwrap(),
                Regex::new(r"AKIA[A-Z0-9]{16}").unwrap(),
                Regex::new(r"ghp_[a-zA-Z0-9]{36,}").unwrap(),
                Regex::new(r"gho_[a-zA-Z0-9]{36,}").unwrap(),
                Regex::new(r"Bearer\s+[a-zA-Z0-9._\-]{20,}").unwrap(),
                Regex::new(r#"(?i)password\s*[=:]\s*[^\s"]+"#).unwrap(),
                Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----").unwrap(),
                Regex::new(r"eyJ[a-zA-Z0-9_\-]{20,}\.eyJ[a-zA-Z0-9_\-]{20,}").unwrap(),
            ]
        } else {
            vec![]
        };
        Self { patterns, enabled }
    }

    pub fn redact(&self, text: &str) -> (String, usize) {
        if !self.enabled {
            return (text.to_string(), 0);
        }

        let mut result = text.to_string();
        let mut count = 0;

        for pattern in &self.patterns {
            let matches: Vec<_> = pattern.find_iter(&result).collect();
            count += matches.len();
            result = pattern.replace_all(&result, "[REDACTED]").into_owned();
        }

        (result, count)
    }

    /// 对结构化 `serde_json::Value` 递归脱敏，返回 `(脱敏后的值, 命中总数)`。
    ///
    /// 只替换字符串**叶子值**与对象 **key** 内的 secret 子串，结构字符
    /// (`{}`/`[]`/`"`/`,`/`:`) 永不进入替换目标——从根因上杜绝脱敏破坏 JSON 结构
    /// （对比旧的「序列化后对文本正则」实现，见 change `mcp-redact-preserve-json-structure`）。
    /// 对象 key 一并脱敏是因为 `get_tool_output` 的 `Structured` 输出原样保留任意
    /// 工具 JSON，其 key 可能含用户数据（codex design 二审 finding）。
    pub fn redact_value(&self, value: serde_json::Value) -> (serde_json::Value, usize) {
        if !self.enabled {
            return (value, 0);
        }
        match value {
            serde_json::Value::String(s) => {
                let (red, n) = self.redact(&s);
                (serde_json::Value::String(red), n)
            }
            serde_json::Value::Array(arr) => {
                let mut count = 0;
                let out = arr
                    .into_iter()
                    .map(|v| {
                        let (rv, n) = self.redact_value(v);
                        count += n;
                        rv
                    })
                    .collect();
                (serde_json::Value::Array(out), count)
            }
            serde_json::Value::Object(map) => {
                let mut count = 0;
                let mut out = serde_json::Map::with_capacity(map.len());
                for (k, v) in map {
                    let (rk, kn) = self.redact(&k);
                    let (rv, vn) = self.redact_value(v);
                    count += kn + vn;
                    out.insert(rk, rv);
                }
                (serde_json::Value::Object(out), count)
            }
            other => (other, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_anthropic_api_key() {
        let r = Redactor::new(true);
        let (out, count) = r.redact("key: sk-ant-api03-abcdefghijklmnopqrstuvwxyz");
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("sk-ant-api03"));
        assert_eq!(count, 1);
    }

    #[test]
    fn redacts_aws_key() {
        let r = Redactor::new(true);
        let (out, count) = r.redact("AWS_KEY=AKIAIOSFODNN7EXAMPLE");
        assert!(out.contains("[REDACTED]"));
        assert_eq!(count, 1);
    }

    #[test]
    fn redacts_github_pat() {
        let r = Redactor::new(true);
        let (out, count) = r.redact("token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijkl");
        assert!(out.contains("[REDACTED]"));
        assert_eq!(count, 1);
    }

    #[test]
    fn redacts_bearer_token() {
        let r = Redactor::new(true);
        let (out, count) =
            r.redact("Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.long");
        assert!(out.contains("[REDACTED]"));
        assert!(count >= 1);
    }

    #[test]
    fn redacts_password_assignment() {
        let r = Redactor::new(true);
        let (out, count) = r.redact("password=s3cr3t_value_here");
        assert!(out.contains("[REDACTED]"));
        assert_eq!(count, 1);
    }

    #[test]
    fn disabled_redactor_passes_through() {
        let r = Redactor::new(false);
        let (out, count) = r.redact("sk-ant-api03-abcdefghijklmnopqrstuvwxyz");
        assert!(out.contains("sk-ant-api03"));
        assert_eq!(count, 0);
    }

    #[test]
    fn multiple_secrets_in_one_text() {
        let r = Redactor::new(true);
        let text = "key1=sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaa key2=AKIAIOSFODNN7EXAMPLE";
        let (out, count) = r.redact(text);
        assert_eq!(count, 2);
        assert!(!out.contains("sk-ant"));
        assert!(!out.contains("AKIA"));
    }

    // ── 结构化递归脱敏（change mcp-redact-preserve-json-structure）──

    #[test]
    fn redact_value_preserves_json_structure() {
        let r = Redactor::new(true);
        let v = serde_json::json!({
            "text": "run with password=hunter2",
            "model": "claude-opus",
            "cost": 1.5
        });
        let (out, count) = r.redact_value(v);
        assert!(count >= 1);
        // 其余字段完整保留、值不被截断
        assert_eq!(out["model"], "claude-opus");
        assert_eq!(out["cost"], 1.5);
        let text = out["text"].as_str().unwrap();
        assert!(text.contains("[REDACTED]"));
        assert!(!text.contains("hunter2"));
    }

    #[test]
    fn redact_value_password_no_longer_eats_across_fields() {
        // 回归 #596:结构化脱敏下,一个字段的 password= 不会吞掉相邻字段
        let r = Redactor::new(true);
        let v = serde_json::json!({ "a": "password=hunter2", "b": "keepme" });
        let (out, _count) = r.redact_value(v);
        assert_eq!(out["b"], "keepme");
        assert!(!out["a"].as_str().unwrap().contains("hunter2"));
    }

    #[test]
    fn redact_value_recurses_nested_and_arrays() {
        let r = Redactor::new(true);
        let v = serde_json::json!({
            "outer": { "inner": "token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijkl" },
            "list": ["AKIAIOSFODNN7EXAMPLE", "safe value"]
        });
        let (out, count) = r.redact_value(v);
        assert_eq!(count, 2);
        assert!(
            out["outer"]["inner"]
                .as_str()
                .unwrap()
                .contains("[REDACTED]")
        );
        assert!(out["list"][0].as_str().unwrap().contains("[REDACTED]"));
        assert_eq!(out["list"][1], "safe value");
    }

    #[test]
    fn redact_value_redacts_object_keys() {
        // Structured tool output 原样保留任意 JSON,secret 可能出现在 key 上（codex finding）
        let r = Redactor::new(true);
        let v = serde_json::json!({ "AKIAIOSFODNN7EXAMPLE": "value" });
        let (out, count) = r.redact_value(v);
        assert_eq!(count, 1);
        let obj = out.as_object().unwrap();
        assert!(obj.contains_key("[REDACTED]"));
        assert!(!obj.keys().any(|k| k.contains("AKIA")));
    }

    #[test]
    fn redact_value_disabled_passthrough() {
        let r = Redactor::new(false);
        let v = serde_json::json!({ "text": "password=s3cret" });
        let (out, count) = r.redact_value(v.clone());
        assert_eq!(count, 0);
        assert_eq!(out, v);
    }

    #[test]
    fn redact_value_non_string_leaves_untouched() {
        let r = Redactor::new(true);
        let v = serde_json::json!({ "n": 42, "b": true, "nil": null });
        let (out, count) = r.redact_value(v.clone());
        assert_eq!(count, 0);
        assert_eq!(out, v);
    }
}
