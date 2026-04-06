use once_cell::sync::Lazy;
use regex::Regex;

/// Each entry is (pattern, replacement). The replacement may use `$1`, `$2`, etc.
/// to preserve surrounding context while redacting only the sensitive value.
static PATTERNS: Lazy<Vec<(Regex, &str)>> = Lazy::new(|| {
    vec![
        // OpenAI / Anthropic / generic sk- tokens
        (Regex::new(r"sk-[A-Za-z0-9_\-]{10,}").unwrap(), "[REDACTED]"),
        // GitHub tokens
        (Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(), "[REDACTED]"),
        (Regex::new(r"github_pat_[A-Za-z0-9_]{82}").unwrap(), "[REDACTED]"),
        // JWTs (three base64url segments separated by dots)
        (Regex::new(r"eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+").unwrap(), "[REDACTED]"),
        // PEM private key blocks
        (Regex::new(r"-----BEGIN [A-Z ]+-----[\s\S]*?-----END [A-Z ]+-----").unwrap(), "[REDACTED]"),
        // Environment variable assignments with token-like values (20+ chars, no spaces).
        // Group 1 captures the key+assignment so it is preserved in output.
        (
            Regex::new(r#"(?i)((?:token|secret|password|key|auth|api_key)\s*=\s*['"]?)[A-Za-z0-9_\-\.]{20,}['"]?"#).unwrap(),
            "${1}[REDACTED]",
        ),
    ]
});

pub fn sanitize(input: &str) -> String {
    let mut output = input.to_string();
    for (pattern, replacement) in PATTERNS.iter() {
        output = pattern.replace_all(&output, *replacement).into_owned();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sk_token() {
        let s = sanitize("use sk-abc123XYZabc123XYZabc for the API");
        assert!(!s.contains("sk-abc"), "sk- token should be redacted: {s}");
        assert!(s.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_jwt() {
        let s = sanitize("token: eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c");
        assert!(!s.contains("eyJ"), "JWT should be redacted: {s}");
    }

    #[test]
    fn redacts_pem_block() {
        let s = sanitize("key: -----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAK\n-----END RSA PRIVATE KEY-----");
        assert!(!s.contains("MIIEowIBAAK"), "PEM block should be redacted: {s}");
    }

    #[test]
    fn leaves_normal_text_unchanged() {
        let s = sanitize("tower-sessions used for auth, not JWT libraries");
        assert_eq!(s, "tower-sessions used for auth, not JWT libraries");
    }

    #[test]
    fn env_var_redaction_preserves_key_name() {
        let s = sanitize("TOKEN=supersecretvalue1234567");
        assert!(s.contains("TOKEN="), "key name should be preserved: {s}");
        assert!(!s.contains("supersecretvalue"), "value should be redacted: {s}");
        assert!(s.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_github_token() {
        // Pattern requires exactly 36 alphanumeric chars after ghp_
        let s = sanitize("auth: ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZabcd123456");
        assert!(!s.contains("ghp_"), "GitHub token should be redacted: {s}");
        assert!(s.contains("[REDACTED]"));
    }

    #[test]
    fn short_env_var_value_not_redacted() {
        let s = sanitize("TOKEN=short");
        assert_eq!(s, "TOKEN=short", "short values should not be redacted");
    }
}
