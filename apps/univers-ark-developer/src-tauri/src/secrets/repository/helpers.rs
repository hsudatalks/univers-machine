use crate::models::{SecretAssignmentTargetKind, SecretCredentialRecord};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn database_file_name() -> &'static str {
    if cfg!(debug_assertions) {
        "univers-ark-developer.dev.db"
    } else {
        "univers-ark-developer.db"
    }
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn normalize_identifier(value: &str, fallback_prefix: &str) -> String {
    let mut identifier = String::new();
    let mut previous_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            identifier.push(ch.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator {
            identifier.push('-');
            previous_separator = true;
        }
    }

    let trimmed = identifier.trim_matches('-');

    if trimmed.is_empty() {
        format!("{}-{}", fallback_prefix, now_ms())
    } else {
        trimmed.to_string()
    }
}

pub(super) fn generated_id(prefix: &str, label: &str, current_id: &str) -> String {
    if !current_id.trim().is_empty() {
        return normalize_identifier(current_id, prefix);
    }

    let normalized_label = normalize_identifier(label, prefix);
    format!("{}-{}", normalized_label, now_ms())
}

pub(super) fn format_target_kind(value: SecretAssignmentTargetKind) -> &'static str {
    match value {
        SecretAssignmentTargetKind::Machine => "machine",
        SecretAssignmentTargetKind::Container => "container",
    }
}

pub(super) fn parse_target_kind(value: &str) -> rusqlite::Result<SecretAssignmentTargetKind> {
    match value {
        "machine" => Ok(SecretAssignmentTargetKind::Machine),
        "container" => Ok(SecretAssignmentTargetKind::Container),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown assignment target kind {other}"),
            )),
        )),
    }
}

#[derive(Debug)]
pub(super) struct StoredCredentialRecord {
    pub(super) id: String,
    pub(super) provider_id: String,
    pub(super) label: String,
    pub(super) description: String,
    pub(super) secret_backend: String,
    pub(super) secret_account: String,
    pub(super) has_secret: bool,
    pub(super) created_at_ms: u64,
    pub(super) updated_at_ms: u64,
    pub(super) last_rotated_at_ms: Option<u64>,
}

impl StoredCredentialRecord {
    pub(super) fn into_public(self) -> SecretCredentialRecord {
        SecretCredentialRecord {
            id: self.id,
            provider_id: self.provider_id,
            label: self.label,
            description: self.description,
            has_secret: self.has_secret,
            secret_backend: self.secret_backend,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
            last_rotated_at_ms: self.last_rotated_at_ms,
        }
    }
}
