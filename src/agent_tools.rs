use std::time::SystemTime;

use crate::command::RiskLevel;

#[derive(Debug, Clone)]
pub struct PendingCommand {
  pub command: String,
  pub explanation: Option<String>,
  pub risk_level: RiskLevel,
  pub target: CommandTarget,
  pub requested_at: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTarget {
  Shell,
}

impl PendingCommand {
  pub fn new(
    command: String,
    explanation: Option<String>,
    risk_level: RiskLevel,
  ) -> Self {
    Self {
      command,
      explanation,
      risk_level,
      target: CommandTarget::Shell,
      requested_at: SystemTime::now(),
    }
  }
}
