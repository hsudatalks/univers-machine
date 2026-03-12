use super::{
    helpers::{parse_target_kind, StoredCredentialRecord},
    SqliteSecretRepository,
};
use crate::models::{
    SecretAssignmentRecord, SecretAuditEventRecord, SecretInventory, SecretManagementDiagnostics,
    SecretProviderRecord,
};
use rusqlite::{Connection, OptionalExtension};

impl SqliteSecretRepository {
    pub(super) fn load_inventory(&self, store_backend: &str) -> Result<SecretInventory, String> {
        let connection = self.connect()?;
        let providers = self.list_providers(&connection)?;
        let credentials = self
            .list_credentials(&connection)?
            .into_iter()
            .map(StoredCredentialRecord::into_public)
            .collect();
        let assignments = self.list_assignments(&connection)?;
        let audit_events = self.list_audit_events(&connection)?;

        Ok(SecretInventory {
            db_path: self.sqlite.path().display().to_string(),
            store_backend: store_backend.to_string(),
            providers,
            credentials,
            assignments,
            audit_events,
        })
    }

    pub(super) fn diagnostics(
        &self,
        store_backend: &str,
    ) -> Result<SecretManagementDiagnostics, String> {
        let connection = self.connect()?;
        let provider_count = self.scalar_count(&connection, "secret_providers")?;
        let credential_count = self.scalar_count(&connection, "secret_credentials")?;
        let assignment_count = self.scalar_count(&connection, "secret_assignments")?;
        let audit_event_count = self.scalar_count(&connection, "secret_audit_events")?;

        Ok(SecretManagementDiagnostics {
            db_path: self.sqlite.path().display().to_string(),
            store_backend: store_backend.to_string(),
            provider_count,
            credential_count,
            assignment_count,
            audit_event_count,
        })
    }

    fn scalar_count(&self, connection: &Connection, table_name: &str) -> Result<usize, String> {
        let sql = format!("SELECT COUNT(*) FROM {}", table_name);
        connection
            .query_row(&sql, [], |row| row.get::<_, i64>(0))
            .map(|value| value.max(0) as usize)
            .map_err(|error| format!("Failed to inspect {}: {}", table_name, error))
    }

    pub(super) fn list_providers(
        &self,
        connection: &Connection,
    ) -> Result<Vec<SecretProviderRecord>, String> {
        let mut statement = connection
            .prepare(
                "SELECT id, label, provider_kind, base_url, description, created_at_ms, updated_at_ms
                 FROM secret_providers
                 ORDER BY label COLLATE NOCASE, id",
            )
            .map_err(|error| format!("Failed to query secret providers: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                Ok(SecretProviderRecord {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    provider_kind: row.get(2)?,
                    base_url: row.get(3)?,
                    description: row.get(4)?,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                })
            })
            .map_err(|error| format!("Failed to map secret providers: {}", error))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to read secret providers: {}", error))
    }

    pub(super) fn list_credentials(
        &self,
        connection: &Connection,
    ) -> Result<Vec<StoredCredentialRecord>, String> {
        let mut statement = connection
            .prepare(
                "SELECT id, provider_id, label, description, secret_backend, secret_account,
                        secret_present, created_at_ms, updated_at_ms, last_rotated_at_ms
                 FROM secret_credentials
                 ORDER BY label COLLATE NOCASE, id",
            )
            .map_err(|error| format!("Failed to query secret credentials: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                let secret_present: i64 = row.get(6)?;
                Ok(StoredCredentialRecord {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    label: row.get(2)?,
                    description: row.get(3)?,
                    secret_backend: row.get(4)?,
                    secret_account: row.get(5)?,
                    has_secret: secret_present != 0,
                    created_at_ms: row.get(7)?,
                    updated_at_ms: row.get(8)?,
                    last_rotated_at_ms: row.get(9)?,
                })
            })
            .map_err(|error| format!("Failed to map secret credentials: {}", error))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to read secret credentials: {}", error))
    }

    pub(super) fn list_assignments(
        &self,
        connection: &Connection,
    ) -> Result<Vec<SecretAssignmentRecord>, String> {
        let mut statement = connection
            .prepare(
                "SELECT id, credential_id, target_kind, target_id, env_var, file_path,
                        enabled, created_at_ms, updated_at_ms
                 FROM secret_assignments
                 ORDER BY target_kind, target_id, id",
            )
            .map_err(|error| format!("Failed to query secret assignments: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                let target_kind: String = row.get(2)?;
                let enabled: i64 = row.get(6)?;
                Ok(SecretAssignmentRecord {
                    id: row.get(0)?,
                    credential_id: row.get(1)?,
                    target_kind: parse_target_kind(&target_kind)?,
                    target_id: row.get(3)?,
                    env_var: row.get(4)?,
                    file_path: row.get(5)?,
                    enabled: enabled != 0,
                    created_at_ms: row.get(7)?,
                    updated_at_ms: row.get(8)?,
                })
            })
            .map_err(|error| format!("Failed to map secret assignments: {}", error))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to read secret assignments: {}", error))
    }

    pub(super) fn list_audit_events(
        &self,
        connection: &Connection,
    ) -> Result<Vec<SecretAuditEventRecord>, String> {
        let mut statement = connection
            .prepare(
                "SELECT id, event_kind, entity_kind, entity_id, detail, created_at_ms
                 FROM secret_audit_events
                 ORDER BY created_at_ms DESC, id DESC
                 LIMIT 20",
            )
            .map_err(|error| format!("Failed to query secret audit events: {}", error))?;

        let rows = statement
            .query_map([], |row| {
                Ok(SecretAuditEventRecord {
                    id: row.get(0)?,
                    event_kind: row.get(1)?,
                    entity_kind: row.get(2)?,
                    entity_id: row.get(3)?,
                    detail: row.get(4)?,
                    created_at_ms: row.get(5)?,
                })
            })
            .map_err(|error| format!("Failed to map secret audit events: {}", error))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to read secret audit events: {}", error))
    }

    pub(super) fn get_provider(
        &self,
        connection: &Connection,
        provider_id: &str,
    ) -> Result<Option<SecretProviderRecord>, String> {
        connection
            .query_row(
                "SELECT id, label, provider_kind, base_url, description, created_at_ms, updated_at_ms
                 FROM secret_providers
                 WHERE id = ?1",
                [provider_id],
                |row| {
                    Ok(SecretProviderRecord {
                        id: row.get(0)?,
                        label: row.get(1)?,
                        provider_kind: row.get(2)?,
                        base_url: row.get(3)?,
                        description: row.get(4)?,
                        created_at_ms: row.get(5)?,
                        updated_at_ms: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("Failed to inspect secret provider {}: {}", provider_id, error))
    }

    pub(super) fn get_credential(
        &self,
        connection: &Connection,
        credential_id: &str,
    ) -> Result<Option<StoredCredentialRecord>, String> {
        connection
            .query_row(
                "SELECT id, provider_id, label, description, secret_backend, secret_account,
                        secret_present, created_at_ms, updated_at_ms, last_rotated_at_ms
                 FROM secret_credentials
                 WHERE id = ?1",
                [credential_id],
                |row| {
                    let secret_present: i64 = row.get(6)?;
                    Ok(StoredCredentialRecord {
                        id: row.get(0)?,
                        provider_id: row.get(1)?,
                        label: row.get(2)?,
                        description: row.get(3)?,
                        secret_backend: row.get(4)?,
                        secret_account: row.get(5)?,
                        has_secret: secret_present != 0,
                        created_at_ms: row.get(7)?,
                        updated_at_ms: row.get(8)?,
                        last_rotated_at_ms: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|error| {
                format!(
                    "Failed to inspect secret credential {}: {}",
                    credential_id, error
                )
            })
    }
}
