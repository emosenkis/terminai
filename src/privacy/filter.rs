use regex::Regex;

/// Privacy filter to redact sensitive information from terminal output
pub struct PrivacyFilter {
    patterns: Vec<(Regex, &'static str)>,
}

impl PrivacyFilter {
    pub fn new() -> Self {
        let patterns = vec![
            // API keys and tokens
            (
                Regex::new(r"(?i)(api[_-]?key|apikey)\s*[=:]\s*['\"]?([a-zA-Z0-9_\-]{16,})['\"]?")
                    .unwrap(),
                "[REDACTED_API_KEY]",
            ),
            (
                Regex::new(r"(?i)(token|access[_-]?token)\s*[=:]\s*['\"]?([a-zA-Z0-9_\-\.]{16,})['\"]?")
                    .unwrap(),
                "[REDACTED_TOKEN]",
            ),
            // Passwords
            (
                Regex::new(r"(?i)(password|passwd|pwd)\s*[=:]\s*['\"]?([^\s'\";]{8,})['\"]?")
                    .unwrap(),
                "[REDACTED_PASSWORD]",
            ),
            // AWS credentials
            (
                Regex::new(r"(?i)(AWS_ACCESS_KEY_ID|AWS_SECRET_ACCESS_KEY)\s*[=:]\s*['\"]?([A-Za-z0-9/+=]{16,})['\"]?")
                    .unwrap(),
                "[REDACTED_AWS_KEY]",
            ),
            // SSH private keys (header)
            (
                Regex::new(r"-----BEGIN [A-Z\s]+ PRIVATE KEY-----[\s\S]*?-----END [A-Z\s]+ PRIVATE KEY-----")
                    .unwrap(),
                "[REDACTED_PRIVATE_KEY]",
            ),
            // Email addresses (optional - can be disabled)
            (
                Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")
                    .unwrap(),
                "[REDACTED_EMAIL]",
            ),
            // Credit card numbers (basic pattern)
            (
                Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b")
                    .unwrap(),
                "[REDACTED_CARD]",
            ),
            // Generic secrets
            (
                Regex::new(r"(?i)(secret|secret[_-]?key)\s*[=:]\s*['\"]?([a-zA-Z0-9_\-]{16,})['\"]?")
                    .unwrap(),
                "[REDACTED_SECRET]",
            ),
            // JWT tokens
            (
                Regex::new(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}")
                    .unwrap(),
                "[REDACTED_JWT]",
            ),
            // Database connection strings
            (
                Regex::new(r"(?i)(postgres|mysql|mongodb)://[^:]+:[^@]+@[\w\.\-:]+/?\S*")
                    .unwrap(),
                "[REDACTED_DB_URI]",
            ),
        ];

        Self { patterns }
    }

    /// Filter text and redact sensitive information
    pub fn filter(&self, text: &str) -> String {
        let mut filtered = text.to_string();

        for (pattern, replacement) in &self.patterns {
            filtered = pattern.replace_all(&filtered, *replacement).to_string();
        }

        filtered
    }

    /// Filter multiple lines
    pub fn filter_lines(&self, lines: &[String]) -> Vec<String> {
        lines.iter().map(|line| self.filter(line)).collect()
    }

    /// Check if text contains sensitive information
    pub fn contains_sensitive(&self, text: &str) -> bool {
        self.patterns.iter().any(|(pattern, _)| pattern.is_match(text))
    }
}

impl Default for PrivacyFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_api_key() {
        let filter = PrivacyFilter::new();

        let text = "API_KEY=sk-1234567890abcdef1234567890";
        let filtered = filter.filter(text);
        assert!(filtered.contains("[REDACTED_API_KEY]"));
        assert!(!filtered.contains("sk-1234567890"));
    }

    #[test]
    fn test_filter_password() {
        let filter = PrivacyFilter::new();

        let text = "password=mysecretpass123";
        let filtered = filter.filter(text);
        assert!(filtered.contains("[REDACTED_PASSWORD]"));
        assert!(!filtered.contains("mysecretpass123"));
    }

    #[test]
    fn test_filter_aws_credentials() {
        let filter = PrivacyFilter::new();

        let text = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let filtered = filter.filter(text);
        assert!(filtered.contains("[REDACTED_AWS_KEY]"));
        assert!(!filtered.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_filter_email() {
        let filter = PrivacyFilter::new();

        let text = "Contact me at user@example.com";
        let filtered = filter.filter(text);
        assert!(filtered.contains("[REDACTED_EMAIL]"));
        assert!(!filtered.contains("user@example.com"));
    }

    #[test]
    fn test_filter_jwt() {
        let filter = PrivacyFilter::new();

        let text = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let filtered = filter.filter(text);
        assert!(filtered.contains("[REDACTED_JWT]"));
        assert!(!filtered.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn test_filter_multiple_secrets() {
        let filter = PrivacyFilter::new();

        let text = "api_key=sk-123456 password=secret123 token=xyz789abc";
        let filtered = filter.filter(text);
        assert!(filtered.contains("[REDACTED_API_KEY]"));
        assert!(filtered.contains("[REDACTED_PASSWORD]"));
        assert!(filtered.contains("[REDACTED_TOKEN]"));
    }

    #[test]
    fn test_filter_lines() {
        let filter = PrivacyFilter::new();

        let lines = vec![
            "Normal line".to_string(),
            "API_KEY=secret123".to_string(),
            "Another normal line".to_string(),
        ];

        let filtered = filter.filter_lines(&lines);
        assert_eq!(filtered[0], "Normal line");
        assert!(filtered[1].contains("[REDACTED_API_KEY]"));
        assert_eq!(filtered[2], "Another normal line");
    }

    #[test]
    fn test_contains_sensitive() {
        let filter = PrivacyFilter::new();

        assert!(filter.contains_sensitive("password=secret123"));
        assert!(filter.contains_sensitive("api_key=sk-123456"));
        assert!(!filter.contains_sensitive("just normal text"));
    }

    #[test]
    fn test_no_false_positives() {
        let filter = PrivacyFilter::new();

        // These should not be filtered
        let text = "The API is working fine";
        let filtered = filter.filter(text);
        assert_eq!(filtered, text);
    }

    #[test]
    fn test_database_uri() {
        let filter = PrivacyFilter::new();

        let text = "postgres://user:pass@localhost:5432/mydb";
        let filtered = filter.filter(text);
        assert!(filtered.contains("[REDACTED_DB_URI]"));
        assert!(!filtered.contains("user:pass"));
    }
}
