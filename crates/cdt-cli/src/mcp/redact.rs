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
                Regex::new(r"(?i)password\s*[=:]\s*\S+").unwrap(),
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
        let (out, count) =
            r.redact("token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijkl");
        assert!(out.contains("[REDACTED]"));
        assert_eq!(count, 1);
    }

    #[test]
    fn redacts_bearer_token() {
        let r = Redactor::new(true);
        let (out, count) = r.redact("Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.long");
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
}
