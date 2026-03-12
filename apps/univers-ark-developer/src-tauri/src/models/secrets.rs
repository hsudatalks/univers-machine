use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SecretAssignmentTargetKind {
    Machine,
    Container,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretProviderRecord {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) provider_kind: String,
    pub(crate) base_url: String,
    pub(crate) description: String,
    pub(crate) created_at_ms: u64,
    pub(crate) updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretCredentialRecord {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) has_secret: bool,
    pub(crate) secret_backend: String,
    pub(crate) created_at_ms: u64,
    pub(crate) updated_at_ms: u64,
    pub(crate) last_rotated_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretAssignmentRecord {
    pub(crate) id: String,
    pub(crate) credential_id: String,
    pub(crate) target_kind: SecretAssignmentTargetKind,
    pub(crate) target_id: String,
    pub(crate) env_var: String,
    pub(crate) file_path: String,
    pub(crate) enabled: bool,
    pub(crate) created_at_ms: u64,
    pub(crate) updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretAuditEventRecord {
    pub(crate) id: i64,
    pub(crate) event_kind: String,
    pub(crate) entity_kind: String,
    pub(crate) entity_id: String,
    pub(crate) detail: String,
    pub(crate) created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretInventory {
    pub(crate) db_path: String,
    pub(crate) store_backend: String,
    pub(crate) providers: Vec<SecretProviderRecord>,
    pub(crate) credentials: Vec<SecretCredentialRecord>,
    pub(crate) assignments: Vec<SecretAssignmentRecord>,
    pub(crate) audit_events: Vec<SecretAuditEventRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretProviderInput {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) provider_kind: String,
    #[serde(default)]
    pub(crate) base_url: String,
    #[serde(default)]
    pub(crate) description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretCredentialInput {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) secret_value: Option<String>,
    #[serde(default)]
    pub(crate) clear_secret: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretAssignmentInput {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) credential_id: String,
    pub(crate) target_kind: SecretAssignmentTargetKind,
    pub(crate) target_id: String,
    #[serde(default)]
    pub(crate) env_var: String,
    #[serde(default)]
    pub(crate) file_path: String,
    #[serde(default = "default_secret_assignment_enabled")]
    pub(crate) enabled: bool,
}

fn default_secret_assignment_enabled() -> bool {
    true
}
