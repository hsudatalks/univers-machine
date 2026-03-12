use crate::models::{
    SecretAssignmentInput, SecretAssignmentRecord, SecretCredentialInput, SecretCredentialRecord,
    SecretInventory, SecretManagementDiagnostics, SecretProviderInput, SecretProviderRecord,
};

const MOBILE_UNSUPPORTED_MESSAGE: &str =
    "Secret management is not available on mobile yet.";

#[derive(Clone, Default)]
pub(crate) struct SecretManagementState;

impl SecretManagementState {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub(crate) fn load_inventory(&self) -> Result<SecretInventory, String> {
        Ok(SecretInventory {
            db_path: String::from("mobile-unavailable"),
            store_backend: String::from("unsupported-mobile"),
            providers: Vec::new(),
            credentials: Vec::new(),
            assignments: Vec::new(),
            audit_events: Vec::new(),
        })
    }

    pub(crate) fn diagnostics(&self) -> Result<SecretManagementDiagnostics, String> {
        Ok(SecretManagementDiagnostics {
            db_path: String::from("mobile-unavailable"),
            store_backend: String::from("unsupported-mobile"),
            provider_count: 0,
            credential_count: 0,
            assignment_count: 0,
            audit_event_count: 0,
        })
    }

    pub(crate) fn upsert_provider(
        &self,
        _input: SecretProviderInput,
    ) -> Result<SecretProviderRecord, String> {
        Err(String::from(MOBILE_UNSUPPORTED_MESSAGE))
    }

    pub(crate) fn delete_provider(&self, _provider_id: &str) -> Result<(), String> {
        Err(String::from(MOBILE_UNSUPPORTED_MESSAGE))
    }

    pub(crate) fn upsert_credential(
        &self,
        _input: SecretCredentialInput,
    ) -> Result<SecretCredentialRecord, String> {
        Err(String::from(MOBILE_UNSUPPORTED_MESSAGE))
    }

    pub(crate) fn delete_credential(&self, _credential_id: &str) -> Result<(), String> {
        Err(String::from(MOBILE_UNSUPPORTED_MESSAGE))
    }

    pub(crate) fn upsert_assignment(
        &self,
        _input: SecretAssignmentInput,
    ) -> Result<SecretAssignmentRecord, String> {
        Err(String::from(MOBILE_UNSUPPORTED_MESSAGE))
    }

    pub(crate) fn delete_assignment(&self, _assignment_id: &str) -> Result<(), String> {
        Err(String::from(MOBILE_UNSUPPORTED_MESSAGE))
    }
}
