use std::{collections::HashSet, sync::OnceLock};

use anyhow::{Result, bail};
use redact_core::{
  AnalyzerEngine, AnonymizationStrategy, AnonymizerConfig, EntityType,
  gitleaks_entity_types,
};

use crate::terminai_config::{PrivacyConfig, PrivacyStrategy};

/// Pattern-only privacy filter for terminal output returned to agents.
pub struct PrivacyFilter {
  entity_types: Vec<EntityType>,
  anonymizer_config: AnonymizerConfig,
}

static ANALYZER_ENGINE: OnceLock<AnalyzerEngine> = OnceLock::new();
static ANALYZER_ENGINE_WARMUP: OnceLock<()> = OnceLock::new();

impl PrivacyFilter {
  pub fn new() -> Self {
    Self::from_config(&PrivacyConfig::default())
      .expect("the built-in privacy configuration must be valid")
  }

  pub fn from_config(config: &PrivacyConfig) -> Result<Self> {
    let entity_types = resolve_patterns(&config.patterns)?;
    start_engine_warmup();
    Ok(Self {
      entity_types,
      anonymizer_config: AnonymizerConfig {
        strategy: match config.strategy {
          PrivacyStrategy::Replace => AnonymizationStrategy::Replace,
          PrivacyStrategy::Mask => AnonymizationStrategy::Mask,
          PrivacyStrategy::Hash => AnonymizationStrategy::Hash,
          PrivacyStrategy::Encrypt => AnonymizationStrategy::Encrypt,
          PrivacyStrategy::Redact => AnonymizationStrategy::Redact,
        },
        ..Default::default()
      },
    })
  }

  async fn engine(&self) -> &'static AnalyzerEngine {
    tokio::task::spawn_blocking(|| {
      ANALYZER_ENGINE.get_or_init(AnalyzerEngine::new)
    })
    .await
    .expect("privacy analyzer initialization task must not panic")
  }

  #[cfg(test)]
  async fn shared_engine_address(&self) -> usize {
    self.engine().await as *const AnalyzerEngine as usize
  }

  /// Filter text. Fail closed if Redact cannot process a matching request.
  pub async fn filter(&self, text: &str) -> String {
    let engine = self.engine().await;
    let Ok(analysis) =
      engine.analyze_with_entities(text, &self.entity_types, None)
    else {
      return "[REDACTION_FAILED]".to_string();
    };

    engine
      .anonymizer_registry()
      .anonymize(text, analysis.detected_entities, &self.anonymizer_config)
      .map(|result| result.text)
      .unwrap_or_else(|_| "[REDACTION_FAILED]".to_string())
  }

  pub async fn filter_lines(&self, lines: &[String]) -> Vec<String> {
    let mut filtered = Vec::with_capacity(lines.len());
    for line in lines {
      filtered.push(self.filter(line).await);
    }
    filtered
  }

  pub async fn contains_sensitive(&self, text: &str) -> bool {
    self
      .engine()
      .await
      .analyze_with_entities(text, &self.entity_types, None)
      .map(|result| !result.detected_entities.is_empty())
      .unwrap_or(true)
  }
}

fn start_engine_warmup() {
  ANALYZER_ENGINE_WARMUP.get_or_init(|| {
    std::thread::spawn(|| {
      ANALYZER_ENGINE.get_or_init(AnalyzerEngine::new);
    });
  });
}

impl Default for PrivacyFilter {
  fn default() -> Self {
    Self::new()
  }
}

fn resolve_patterns(patterns: &[String]) -> Result<Vec<EntityType>> {
  let mut selected = HashSet::new();
  for pattern in patterns {
    let (remove, name) = pattern
      .strip_prefix('-')
      .map(|name| (true, name))
      .unwrap_or((false, pattern.as_str()));
    let types = pattern_set(name)?;
    if remove {
      for entity_type in types {
        selected.remove(&entity_type);
      }
    } else {
      selected.extend(types);
    }
  }

  let mut entity_types: Vec<_> = selected.into_iter().collect();
  entity_types.sort_by(|left, right| left.as_str().cmp(right.as_str()));
  Ok(entity_types)
}

fn pattern_set(name: &str) -> Result<Vec<EntityType>> {
  if let Some(rule_name) = name.strip_prefix("gitleaks-") {
    let entity_type = EntityType::Custom(format!(
      "GITLEAKS_{}",
      rule_name.replace('-', "_").to_uppercase()
    ));
    if gitleaks_entity_types().contains(&entity_type) {
      return Ok(vec![entity_type]);
    }
  }

  let types = match name {
    // Sensible terminal-sharing default: credentials and identifiers that
    // commonly enable access or identify a person. Network and diagnostic
    // values (URLs, IPs, timestamps, MACs, GUIDs, hashes) stay visible.
    "default" => {
      let mut types = vec![
        EntityType::EmailAddress,
        EntityType::PhoneNumber,
        EntityType::CreditCard,
        EntityType::IbanCode,
        EntityType::UsBankNumber,
        EntityType::UsSsn,
        EntityType::UsDriverLicense,
        EntityType::UsPassport,
        EntityType::UkNhs,
        EntityType::UkNino,
        EntityType::UkDriverLicense,
        EntityType::UkPassportNumber,
        EntityType::MedicalLicense,
        EntityType::MedicalRecordNumber,
        EntityType::PassportNumber,
        EntityType::CryptoWallet,
        EntityType::BtcAddress,
        EntityType::EthAddress,
      ];
      types.extend(gitleaks_entity_types());
      types
    }
    "credentials" => {
      let mut types = vec![
        EntityType::CryptoWallet,
        EntityType::BtcAddress,
        EntityType::EthAddress,
      ];
      types.extend(gitleaks_entity_types());
      types
    }
    "gitleaks" | "secrets" => gitleaks_entity_types(),
    "financial" => vec![
      EntityType::CreditCard,
      EntityType::IbanCode,
      EntityType::UsBankNumber,
      EntityType::UkSortCode,
    ],
    "identity" => vec![
      EntityType::EmailAddress,
      EntityType::PhoneNumber,
      EntityType::UsSsn,
      EntityType::UsDriverLicense,
      EntityType::UsPassport,
      EntityType::UkNino,
      EntityType::UkDriverLicense,
      EntityType::UkPassportNumber,
      EntityType::PassportNumber,
    ],
    "medical" => vec![
      EntityType::UkNhs,
      EntityType::MedicalLicense,
      EntityType::MedicalRecordNumber,
    ],
    "crypto" => vec![
      EntityType::CryptoWallet,
      EntityType::BtcAddress,
      EntityType::EthAddress,
    ],
    "contact" => vec![
      EntityType::EmailAddress,
      EntityType::PhoneNumber,
      EntityType::IpAddress,
      EntityType::Url,
      EntityType::DomainName,
    ],
    "technical" => vec![
      EntityType::Guid,
      EntityType::MacAddress,
      EntityType::Md5Hash,
      EntityType::Sha1Hash,
      EntityType::Sha256Hash,
    ],
    "email-address" => vec![EntityType::EmailAddress],
    "phone-number" => vec![EntityType::PhoneNumber],
    "ip-address" => vec![EntityType::IpAddress],
    "url" => vec![EntityType::Url],
    "domain-name" => vec![EntityType::DomainName],
    "credit-card" => vec![EntityType::CreditCard],
    "iban" | "iban-code" => vec![EntityType::IbanCode],
    "us-bank-number" => vec![EntityType::UsBankNumber],
    "us-ssn" => vec![EntityType::UsSsn],
    "us-driver-license" => vec![EntityType::UsDriverLicense],
    "us-passport" => vec![EntityType::UsPassport],
    "uk-nhs" => vec![EntityType::UkNhs],
    "uk-nino" => vec![EntityType::UkNino],
    "uk-driver-license" => vec![EntityType::UkDriverLicense],
    "uk-passport-number" => vec![EntityType::UkPassportNumber],
    "uk-sort-code" => vec![EntityType::UkSortCode],
    "medical-license" => vec![EntityType::MedicalLicense],
    "medical-record-number" => vec![EntityType::MedicalRecordNumber],
    "passport-number" => vec![EntityType::PassportNumber],
    "crypto-wallet" => vec![EntityType::CryptoWallet],
    "btc-address" => vec![EntityType::BtcAddress],
    "eth-address" => vec![EntityType::EthAddress],
    "guid" => vec![EntityType::Guid],
    "mac-address" => vec![EntityType::MacAddress],
    "md5-hash" => vec![EntityType::Md5Hash],
    "sha1-hash" => vec![EntityType::Sha1Hash],
    "sha256-hash" => vec![EntityType::Sha256Hash],
    "us-zip-code" => vec![EntityType::UsZipCode],
    "uk-postcode" => vec![EntityType::UkPostcode],
    "date-time" => vec![EntityType::DateTime],
    "age" => vec![EntityType::Age],
    _ => bail!("unknown privacy pattern or category `{name}`"),
  };
  Ok(types)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn defaults_redact_identifiers_but_preserve_diagnostics() {
    let filter = PrivacyFilter::new();
    let text = "email=user@example.com ssn=123-45-6789 ip=192.0.2.1 url=https://example.com at=2026-07-17 mac=00:11:22:33:44:55";
    let filtered = filter.filter(text).await;

    assert!(!filtered.contains("user@example.com"));
    assert!(!filtered.contains("123-45-6789"));
    assert!(filtered.contains("192.0.2.1"));
    assert!(filtered.contains("https://example.com"));
    assert!(filtered.contains("2026-07-17"));
    assert!(filtered.contains("00:11:22:33:44:55"));
  }

  #[tokio::test]
  async fn removal_applies_after_category_expansion() {
    let filter = PrivacyFilter::from_config(&PrivacyConfig {
      patterns: vec!["default".into(), "-btc-address".into()],
      ..Default::default()
    })
    .unwrap();
    let address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080";

    assert_eq!(filter.filter(address).await, address);
  }

  #[tokio::test]
  async fn filters_share_one_process_wide_recognizer_engine() {
    let default_filter =
      PrivacyFilter::from_config(&PrivacyConfig::default()).unwrap();
    let mask_filter = PrivacyFilter::from_config(&PrivacyConfig {
      strategy: PrivacyStrategy::Mask,
      ..Default::default()
    })
    .unwrap();

    assert_eq!(
      default_filter.shared_engine_address().await,
      mask_filter.shared_engine_address().await,
    );
  }
}
