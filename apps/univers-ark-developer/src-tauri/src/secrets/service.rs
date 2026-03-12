use super::{
    repository::{default_repository, SecretRepository},
    store::{KeyringSecretStore, SecretStore},
};
use crate::models::{
    SecretAssignmentInput, SecretAssignmentRecord, SecretCredentialInput, SecretCredentialRecord,
    SecretInventory, SecretManagementDiagnostics, SecretProviderInput, SecretProviderRecord,
};
use std::sync::{Arc, Mutex};

struct SecretManager {
    repository: Box<dyn SecretRepository>,
    store: Arc<dyn SecretStore>,
}

impl SecretManager {
    fn new() -> Result<Self, String> {
        let repository = default_repository()?;
        let store = Arc::new(KeyringSecretStore::new()) as Arc<dyn SecretStore>;

        Ok(Self { repository, store })
    }

    fn load_inventory(&self) -> Result<SecretInventory, String> {
        self.repository.load_inventory(self.store.backend_name())
    }

    fn diagnostics(&self) -> Result<SecretManagementDiagnostics, String> {
        self.repository.diagnostics(self.store.backend_name())
    }

    fn upsert_provider(&self, input: SecretProviderInput) -> Result<SecretProviderRecord, String> {
        self.repository.upsert_provider(input)
    }

    fn delete_provider(&self, provider_id: &str) -> Result<(), String> {
        self.repository.delete_provider(provider_id)
    }

    fn upsert_credential(
        &self,
        input: SecretCredentialInput,
    ) -> Result<SecretCredentialRecord, String> {
        self.repository
            .upsert_credential(self.store.as_ref(), input)
    }

    fn delete_credential(&self, credential_id: &str) -> Result<(), String> {
        self.repository
            .delete_credential(self.store.as_ref(), credential_id)
    }

    fn upsert_assignment(
        &self,
        input: SecretAssignmentInput,
    ) -> Result<SecretAssignmentRecord, String> {
        self.repository.upsert_assignment(input)
    }

    fn delete_assignment(&self, assignment_id: &str) -> Result<(), String> {
        self.repository.delete_assignment(assignment_id)
    }
}

#[derive(Clone)]
pub(crate) struct SecretManagementState {
    manager: Arc<Mutex<SecretManager>>,
}

impl SecretManagementState {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            manager: Arc::new(Mutex::new(SecretManager::new()?)),
        })
    }

    pub(crate) fn load_inventory(&self) -> Result<SecretInventory, String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .load_inventory()
    }

    pub(crate) fn diagnostics(&self) -> Result<SecretManagementDiagnostics, String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .diagnostics()
    }

    pub(crate) fn upsert_provider(
        &self,
        input: SecretProviderInput,
    ) -> Result<SecretProviderRecord, String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .upsert_provider(input)
    }

    pub(crate) fn delete_provider(&self, provider_id: &str) -> Result<(), String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .delete_provider(provider_id)
    }

    pub(crate) fn upsert_credential(
        &self,
        input: SecretCredentialInput,
    ) -> Result<SecretCredentialRecord, String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .upsert_credential(input)
    }

    pub(crate) fn delete_credential(&self, credential_id: &str) -> Result<(), String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .delete_credential(credential_id)
    }

    pub(crate) fn upsert_assignment(
        &self,
        input: SecretAssignmentInput,
    ) -> Result<SecretAssignmentRecord, String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .upsert_assignment(input)
    }

    pub(crate) fn delete_assignment(&self, assignment_id: &str) -> Result<(), String> {
        self.manager
            .lock()
            .map_err(|_| String::from("Failed to lock secret manager state"))?
            .delete_assignment(assignment_id)
    }
}
