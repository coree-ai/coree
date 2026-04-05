use once_cell::sync::Lazy;
use regex::Regex;

static PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // OpenAI / Anthropic / generic sk- tokens
        Regex::new(r"sk-[A-Za-z0-9_\-]{10,}").unwrap(),
        // GitHub tokens
        Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(),
        Regex::new(r"github_pat_[A-Za-z0-9_]{82}").unwrap(),
        // JWTs (three base64url segments separated by dots)
        Regex::new(r"eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+").unwrap(),
        // PEM private key blocks
        Regex::new(r"-----BEGIN [A-Z ]+-----[\s\S]*?-----END [A-Z ]+-----").unwrap(),
        // Environment variable assignments with token-like values (20+ chars, no spaces)
        Regex::new(r#"(?i)(?:token|secret|password|key|auth|api_key)\s*=\s*['"]?([A-Za-z0-9_\-\.]{20,})['"]?"#).unwrap(),
    ]
});

pub fn sanitize(input: &str) -> String {
    let mut output = input.to_string();
    for pattern in PATTERNS.iter() {
        output = pattern.replace_all(&output, "[REDACTED]").into_owned();
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
}
