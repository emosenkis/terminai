use std::collections::HashSet;

/// Risk level for commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Safe commands that are read-only or harmless
    Safe,
    /// Commands that modify state but are generally safe
    Caution,
    /// Commands that could cause data loss or system damage
    Dangerous,
}

/// Validator for assessing command safety
pub struct SafetyValidator {
    safe_commands: HashSet<String>,
    dangerous_commands: HashSet<String>,
    dangerous_patterns: Vec<String>,
}

impl SafetyValidator {
    pub fn new() -> Self {
        let safe_commands = [
            "ls", "pwd", "cd", "cat", "less", "more", "head", "tail", "grep", "find", "echo",
            "which", "man", "help", "history", "date", "whoami", "hostname", "uname", "df", "du",
            "ps", "top", "htop", "free", "uptime", "w", "who", "id", "groups", "env", "printenv",
            "git status", "git log", "git diff", "git show", "cargo check", "cargo test",
            "cargo build", "npm test", "npm run",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let dangerous_commands = [
            "rm", "rmdir", "dd", "mkfs", "fdisk", "parted", "shred", "wipefs", "kill", "killall",
            "pkill", "shutdown", "reboot", "halt", "poweroff", "init", "systemctl", "service",
            "chmod 777", "chown", "userdel", "groupdel", "passwd", ">", ">>",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let dangerous_patterns = vec![
            "rm -rf".to_string(),
            "rm -fr".to_string(),
            "rm -r".to_string(),
            "rm -f".to_string(),
            "rm .*".to_string(),
            "dd if=".to_string(),
            "mkfs".to_string(),
            "> /dev/".to_string(),
            "chmod -R".to_string(),
            "chmod 777".to_string(),
            "curl | sh".to_string(),
            "wget | sh".to_string(),
            "curl | bash".to_string(),
            "wget | bash".to_string(),
            "sudo rm".to_string(),
            "sudo dd".to_string(),
        ];

        Self {
            safe_commands,
            dangerous_commands,
            dangerous_patterns,
        }
    }

    /// Assess the risk level of a command
    pub fn assess_risk(&self, command: &str) -> RiskLevel {
        let command_lower = command.to_lowercase();

        // Check for dangerous patterns
        for pattern in &self.dangerous_patterns {
            if command_lower.contains(&pattern.to_lowercase()) {
                return RiskLevel::Dangerous;
            }
        }

        // Check for dangerous commands
        for dangerous_cmd in &self.dangerous_commands {
            if command_lower.starts_with(&dangerous_cmd.to_lowercase())
                || command_lower.contains(&format!(" {}", dangerous_cmd.to_lowercase()))
            {
                return RiskLevel::Dangerous;
            }
        }

        // Check for safe commands
        for safe_cmd in &self.safe_commands {
            if command_lower.starts_with(&safe_cmd.to_lowercase()) {
                return RiskLevel::Safe;
            }
        }

        // Check for sudo (usually requires caution)
        if command_lower.starts_with("sudo ") {
            return RiskLevel::Caution;
        }

        // Default to caution for unknown commands
        RiskLevel::Caution
    }

    /// Check if a command should require explicit approval
    pub fn requires_approval(&self, command: &str) -> bool {
        matches!(
            self.assess_risk(command),
            RiskLevel::Caution | RiskLevel::Dangerous
        )
    }

    /// Get a human-readable description of the risk
    pub fn risk_description(&self, risk: RiskLevel) -> &str {
        match risk {
            RiskLevel::Safe => "Safe - Read-only or harmless operation",
            RiskLevel::Caution => "Caution - Modifies system state",
            RiskLevel::Dangerous => "Dangerous - Could cause data loss or system damage",
        }
    }
}

impl Default for SafetyValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_commands() {
        let validator = SafetyValidator::new();

        assert_eq!(validator.assess_risk("ls -la"), RiskLevel::Safe);
        assert_eq!(validator.assess_risk("pwd"), RiskLevel::Safe);
        assert_eq!(validator.assess_risk("cat file.txt"), RiskLevel::Safe);
        assert_eq!(validator.assess_risk("grep pattern file"), RiskLevel::Safe);
    }

    #[test]
    fn test_dangerous_commands() {
        let validator = SafetyValidator::new();

        assert_eq!(validator.assess_risk("rm -rf /"), RiskLevel::Dangerous);
        assert_eq!(validator.assess_risk("rm -f file"), RiskLevel::Dangerous);
        assert_eq!(validator.assess_risk("dd if=/dev/zero of=/dev/sda"), RiskLevel::Dangerous);
        assert_eq!(validator.assess_risk("chmod 777 /etc"), RiskLevel::Dangerous);
    }

    #[test]
    fn test_caution_commands() {
        let validator = SafetyValidator::new();

        assert_eq!(validator.assess_risk("sudo apt update"), RiskLevel::Caution);
        assert_eq!(validator.assess_risk("mkdir newdir"), RiskLevel::Caution);
        assert_eq!(validator.assess_risk("touch newfile"), RiskLevel::Caution);
    }

    #[test]
    fn test_requires_approval() {
        let validator = SafetyValidator::new();

        assert!(!validator.requires_approval("ls"));
        assert!(validator.requires_approval("rm file"));
        assert!(validator.requires_approval("sudo apt install"));
    }

    #[test]
    fn test_piped_dangerous_commands() {
        let validator = SafetyValidator::new();

        assert_eq!(
            validator.assess_risk("curl http://example.com | sh"),
            RiskLevel::Dangerous
        );
        assert_eq!(
            validator.assess_risk("wget -O - http://example.com | bash"),
            RiskLevel::Dangerous
        );
    }

    #[test]
    fn test_case_insensitive() {
        let validator = SafetyValidator::new();

        assert_eq!(validator.assess_risk("LS -LA"), RiskLevel::Safe);
        assert_eq!(validator.assess_risk("RM -RF /"), RiskLevel::Dangerous);
    }
}
