use regex::Regex;
use std::sync::LazyLock;

static GHP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"ghp_[A-Za-z0-9_]{10,}").expect("valid regex"));

static GITHUB_PAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"github_pat_[A-Za-z0-9_]{10,}").expect("valid regex"));

static GLPAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"glpat-[A-Za-z0-9_\-]{10,}").expect("valid regex"));

static SK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sk-[A-Za-z0-9_\-]{10,}").expect("valid regex"));

static BEARER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9\._~\+\/\-]+={0,2}").expect("valid regex")
});

static KEY_VALUE_SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(password|token|secret|api_key)\s*=\s*(?:'[^']*'|"[^"]*"|[^\s&'"]+)"#)
        .expect("valid regex")
});

pub fn redact(input: &str) -> String {
    let s = GHP_RE.replace_all(input, "ghp_***");
    let s = GITHUB_PAT_RE.replace_all(&s, "github_pat_***");
    let s = GLPAT_RE.replace_all(&s, "glpat-***");
    let s = SK_RE.replace_all(&s, "sk-***");
    let s = BEARER_RE.replace_all(&s, "Bearer [REDACTED]");
    let s = KEY_VALUE_SECRET_RE.replace_all(&s, "$1=[REDACTED]");
    s.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_plain_text_unmodified() {
        let text = "Normal commit: feat(auth): add login form and update README.md";
        assert_eq!(redact(text), text);
    }

    #[test]
    fn redacts_github_tokens() {
        let text = "Set GITHUB_TOKEN to ghp_1234567890abcdef_secret and test";
        assert_eq!(redact(text), "Set GITHUB_TOKEN to ghp_*** and test");

        let pat_text = "Key: github_pat_11AAAAAAA_99999999999999999999";
        assert_eq!(redact(pat_text), "Key: github_pat_***");
    }

    #[test]
    fn redacts_gitlab_tokens() {
        let text = "GitLab PAT: glpat-abcdef1234567890xyz_tok";
        assert_eq!(redact(text), "GitLab PAT: glpat-***");
    }

    #[test]
    fn redacts_openai_style_keys() {
        let text = "Export OPENAI_API_KEY=sk-abc123xyz7890qwertyuiop";
        assert_eq!(redact(text), "Export OPENAI_API_KEY=sk-***");
    }

    #[test]
    fn redacts_bearer_tokens() {
        let text = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.xyz.123==";
        assert_eq!(redact(text), "Authorization: Bearer [REDACTED]");
    }

    #[test]
    fn redacts_key_value_secrets() {
        let text = "connecting with password=super_secret_pass&db=release";
        assert_eq!(
            redact(text),
            "connecting with password=[REDACTED]&db=release"
        );

        let env_text = "token=\"xyz_123_abc\" and secret='top_secret'";
        assert_eq!(redact(env_text), "token=[REDACTED] and secret=[REDACTED]");
    }
}
