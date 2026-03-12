mod helpers;
mod mutations;
mod queries;

use super::store::SecretStore;
use crate::{
    infra::sqlite::SqliteStore,
    machine::univers_config_dir,
    models::{
        SecretAssignmentInput, SecretAssignmentRecord, SecretCredentialInput,
        SecretCredentialRecord, SecretInventory, SecretManagementDiagnostics, SecretProviderInput,
        SecretProviderRecord,
    },
};
use rusqlite::Connection;
use std::path::PathBuf;

use self::helpers::database_file_name;

pub(super) trait SecretRepository: Send + Sync {
    fn load_inventory(&self, store_backend: &str) -> Result<SecretInventory, String>;
    fn diagnostics(&self, store_backend: &str) -> Result<SecretManagementDiagnostics, String>;
    fn upsert_provider(&self, input: SecretProviderInput) -> Result<SecretProviderRecord, String>;
    fn delete_provider(&self, provider_id: &str) -> Result<(), String>;
    fn upsert_credential(
        &self,
        store: &dyn SecretStore,
        input: SecretCredentialInput,
    ) -> Result<SecretCredentialRecord, String>;
    fn delete_credential(&self, store: &dyn SecretStore, credential_id: &str)
        -> Result<(), String>;
    fn upsert_assignment(
        &self,
        input: SecretAssignmentInput,
    ) -> Result<SecretAssignmentRecord, String>;
    fn delete_assignment(&self, assignment_id: &str) -> Result<(), String>;
}

pub(super) struct SqliteSecretRepository {
    sqlite: SqliteStore,
}

pub(super) fn default_repository() -> Result<Box<dyn SecretRepository>, String> {
    let config_dir = univers_config_dir()?;
    Ok(Box::new(SqliteSecretRepository::new(
        config_dir.join(database_file_name()),
    )?))
}

impl SqliteSecretRepository {
    pub(super) fn new(path: PathBuf) -> Result<Self, String> {
        let repository = Self {
            sqlite: SqliteStore::new(path)?,
        };
        repository.migrate()?;
        Ok(repository)
    }

    fn connect(&self) -> Result<Connection, String> {
        self.sqlite.connect()
    }

    fn migrate(&self) -> Result<(), String> {
        self.sqlite.migrate(
            "CREATE TABLE IF NOT EXISTS secret_providers (
                    id            TEXT PRIMARY KEY,
                    label         TEXT NOT NULL,
                    provider_kind TEXT NOT NULL,
                    base_url      TEXT NOT NULL DEFAULT '',
                    description   TEXT NOT NULL DEFAULT '',
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS secret_credentials (
                    id                 TEXT PRIMARY KEY,
                    provider_id        TEXT NOT NULL,
                    label              TEXT NOT NULL,
                    description        TEXT NOT NULL DEFAULT '',
                    secret_backend     TEXT NOT NULL DEFAULT '',
                    secret_account     TEXT NOT NULL DEFAULT '',
                    secret_present     INTEGER NOT NULL DEFAULT 0,
                    created_at_ms      INTEGER NOT NULL,
                    updated_at_ms      INTEGER NOT NULL,
                    last_rotated_at_ms INTEGER,
                    FOREIGN KEY(provider_id) REFERENCES secret_providers(id) ON DELETE CASCADE
                );
                CREATE TABLE IF NOT EXISTS secret_assignments (
                    id            TEXT PRIMARY KEY,
                    credential_id TEXT NOT NULL,
                    target_kind   TEXT NOT NULL,
                    target_id     TEXT NOT NULL,
                    env_var       TEXT NOT NULL DEFAULT '',
                    file_path     TEXT NOT NULL DEFAULT '',
                    enabled       INTEGER NOT NULL DEFAULT 1,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    FOREIGN KEY(credential_id) REFERENCES secret_credentials(id) ON DELETE CASCADE
                );
                CREATE TABLE IF NOT EXISTS secret_audit_events (
                    id            INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_kind    TEXT NOT NULL,
                    entity_kind   TEXT NOT NULL,
                    entity_id     TEXT NOT NULL,
                    detail        TEXT NOT NULL DEFAULT '',
                    created_at_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_secret_credentials_provider_id
                    ON secret_credentials(provider_id);
                CREATE INDEX IF NOT EXISTS idx_secret_assignments_target
                    ON secret_assignments(target_kind, target_id);
                CREATE INDEX IF NOT EXISTS idx_secret_assignments_credential
                    ON secret_assignments(credential_id);
                CREATE INDEX IF NOT EXISTS idx_secret_audit_events_created_at
                    ON secret_audit_events(created_at_ms DESC);",
        )
    }
}

impl SecretRepository for SqliteSecretRepository {
    fn load_inventory(&self, store_backend: &str) -> Result<SecretInventory, String> {
        SqliteSecretRepository::load_inventory(self, store_backend)
    }

    fn diagnostics(&self, store_backend: &str) -> Result<SecretManagementDiagnostics, String> {
        SqliteSecretRepository::diagnostics(self, store_backend)
    }

    fn upsert_provider(&self, input: SecretProviderInput) -> Result<SecretProviderRecord, String> {
        SqliteSecretRepository::upsert_provider(self, input)
    }

    fn delete_provider(&self, provider_id: &str) -> Result<(), String> {
        SqliteSecretRepository::delete_provider(self, provider_id)
    }

    fn upsert_credential(
        &self,
        store: &dyn SecretStore,
        input: SecretCredentialInput,
    ) -> Result<SecretCredentialRecord, String> {
        SqliteSecretRepository::upsert_credential(self, store, input)
    }

    fn delete_credential(
        &self,
        store: &dyn SecretStore,
        credential_id: &str,
    ) -> Result<(), String> {
        SqliteSecretRepository::delete_credential(self, store, credential_id)
    }

    fn upsert_assignment(
        &self,
        input: SecretAssignmentInput,
    ) -> Result<SecretAssignmentRecord, String> {
        SqliteSecretRepository::upsert_assignment(self, input)
    }

    fn delete_assignment(&self, assignment_id: &str) -> Result<(), String> {
        SqliteSecretRepository::delete_assignment(self, assignment_id)
    }
}
