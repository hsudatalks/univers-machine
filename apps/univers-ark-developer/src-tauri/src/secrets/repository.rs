use super::store::SecretStore;
use crate::{
    infra::sqlite::SqliteStore,
    machine::univers_config_dir,
    models::{
        SecretAssignmentInput, SecretAssignmentRecord, SecretAssignmentTargetKind,
        SecretAuditEventRecord, SecretCredentialInput, SecretCredentialRecord, SecretInventory,
        SecretManagementDiagnostics, SecretProviderInput, SecretProviderRecord,
    },
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn database_file_name() -> &'static str {
    if cfg!(debug_assertions) {
        "univers-ark-developer.dev.db"
    } else {
        "univers-ark-developer.db"
    }
}

fn now_ms() -> u64 {
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

fn generated_id(prefix: &str, label: &str, current_id: &str) -> String {
    if !current_id.trim().is_empty() {
        return normalize_identifier(current_id, prefix);
    }

    let normalized_label = normalize_identifier(label, prefix);
    format!("{}-{}", normalized_label, now_ms())
}

fn format_target_kind(value: SecretAssignmentTargetKind) -> &'static str {
    match value {
        SecretAssignmentTargetKind::Machine => "machine",
        SecretAssignmentTargetKind::Container => "container",
    }
}

fn parse_target_kind(value: &str) -> rusqlite::Result<SecretAssignmentTargetKind> {
    match value {
        "machine" => Ok(SecretAssignmentTargetKind::Machine),
        "container" => Ok(SecretAssignmentTargetKind::Container),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown assignment target kind {}", other),
            )),
        )),
    }
}

#[derive(Debug)]
struct StoredCredentialRecord {
    id: String,
    provider_id: String,
    label: String,
    description: String,
    secret_backend: String,
    secret_account: String,
    has_secret: bool,
    created_at_ms: u64,
    updated_at_ms: u64,
    last_rotated_at_ms: Option<u64>,
}

impl StoredCredentialRecord {
    fn into_public(self) -> SecretCredentialRecord {
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

impl SqliteSecretRepository {
    fn load_inventory(&self, store_backend: &str) -> Result<SecretInventory, String> {
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

    fn diagnostics(&self, store_backend: &str) -> Result<SecretManagementDiagnostics, String> {
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
}

impl SqliteSecretRepository {
    fn scalar_count(&self, connection: &Connection, table_name: &str) -> Result<usize, String> {
        let sql = format!("SELECT COUNT(*) FROM {}", table_name);
        connection
            .query_row(&sql, [], |row| row.get::<_, i64>(0))
            .map(|value| value.max(0) as usize)
            .map_err(|error| format!("Failed to inspect {}: {}", table_name, error))
    }

    fn list_providers(&self, connection: &Connection) -> Result<Vec<SecretProviderRecord>, String> {
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

    fn list_credentials(
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

    fn list_assignments(
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

    fn list_audit_events(
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

    fn get_provider(
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

    fn get_credential(
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

impl SecretRepository for SqliteSecretRepository {
    fn load_inventory(&self, store_backend: &str) -> Result<SecretInventory, String> {
        SqliteSecretRepository::load_inventory(self, store_backend)
    }

    fn diagnostics(&self, store_backend: &str) -> Result<SecretManagementDiagnostics, String> {
        SqliteSecretRepository::diagnostics(self, store_backend)
    }

    fn upsert_provider(&self, input: SecretProviderInput) -> Result<SecretProviderRecord, String> {
        let connection = self.connect()?;
        let id = generated_id("provider", &input.label, &input.id);
        let label = input.label.trim().to_string();

        if label.is_empty() {
            return Err(String::from("Secret provider label cannot be empty."));
        }

        let provider_kind = if input.provider_kind.trim().is_empty() {
            String::from("custom")
        } else {
            input.provider_kind.trim().to_string()
        };
        let base_url = input.base_url.trim().to_string();
        let description = input.description.trim().to_string();
        let existing = self.get_provider(&connection, &id)?;
        let now = now_ms();
        let created_at_ms = existing
            .as_ref()
            .map(|item| item.created_at_ms)
            .unwrap_or(now);

        connection
            .execute(
                "INSERT INTO secret_providers (
                    id, label, provider_kind, base_url, description, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    label = excluded.label,
                    provider_kind = excluded.provider_kind,
                    base_url = excluded.base_url,
                    description = excluded.description,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    id,
                    label,
                    provider_kind,
                    base_url,
                    description,
                    created_at_ms,
                    now,
                ],
            )
            .map_err(|error| format!("Failed to save secret provider: {}", error))?;

        self.insert_audit_event(
            &connection,
            if existing.is_some() {
                "providerUpdated"
            } else {
                "providerCreated"
            },
            "provider",
            &id,
            &label,
        )?;

        self.get_provider(&connection, &id)?
            .ok_or_else(|| String::from("Failed to reload saved secret provider."))
    }

    fn delete_provider(&self, provider_id: &str) -> Result<(), String> {
        let connection = self.connect()?;
        let deleted = connection
            .execute("DELETE FROM secret_providers WHERE id = ?1", [provider_id])
            .map_err(|error| {
                format!(
                    "Failed to delete secret provider {}: {}",
                    provider_id, error
                )
            })?;

        if deleted == 0 {
            return Err(format!("Secret provider {} does not exist.", provider_id));
        }

        self.insert_audit_event(&connection, "providerDeleted", "provider", provider_id, "")?;
        Ok(())
    }

    fn upsert_credential(
        &self,
        store: &dyn SecretStore,
        input: SecretCredentialInput,
    ) -> Result<SecretCredentialRecord, String> {
        let connection = self.connect()?;
        let provider_id = input.provider_id.trim().to_string();

        if provider_id.is_empty() {
            return Err(String::from(
                "Secret credential providerId cannot be empty.",
            ));
        }
        if self.get_provider(&connection, &provider_id)?.is_none() {
            return Err(format!("Secret provider {} does not exist.", provider_id));
        }

        let id = generated_id("credential", &input.label, &input.id);
        let label = input.label.trim().to_string();

        if label.is_empty() {
            return Err(String::from("Secret credential label cannot be empty."));
        }

        let description = input.description.trim().to_string();
        let existing = self.get_credential(&connection, &id)?;
        let mut secret_backend = existing
            .as_ref()
            .map(|item| item.secret_backend.clone())
            .unwrap_or_default();
        let mut secret_account = existing
            .as_ref()
            .map(|item| item.secret_account.clone())
            .unwrap_or_default();
        let mut has_secret = existing
            .as_ref()
            .map(|item| item.has_secret)
            .unwrap_or(false);
        let mut last_rotated_at_ms = existing.as_ref().and_then(|item| item.last_rotated_at_ms);

        if input.clear_secret {
            if !secret_account.is_empty() {
                let _ = store.delete_secret(&secret_account);
            }
            secret_backend.clear();
            secret_account.clear();
            has_secret = false;
            last_rotated_at_ms = Some(now_ms());
        } else if let Some(secret_value) = input.secret_value.as_ref() {
            let value = secret_value.trim();

            if !value.is_empty() {
                if secret_account.is_empty() {
                    secret_account = format!(
                        "{}:{}",
                        if cfg!(debug_assertions) {
                            "dev"
                        } else {
                            "prod"
                        },
                        id
                    );
                }
                store.set_secret(&secret_account, value)?;
                secret_backend = store.backend_name().to_string();
                has_secret = true;
                last_rotated_at_ms = Some(now_ms());
            }
        }

        let now = now_ms();
        let created_at_ms = existing
            .as_ref()
            .map(|item| item.created_at_ms)
            .unwrap_or(now);

        connection
            .execute(
                "INSERT INTO secret_credentials (
                    id, provider_id, label, description, secret_backend, secret_account,
                    secret_present, created_at_ms, updated_at_ms, last_rotated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    provider_id = excluded.provider_id,
                    label = excluded.label,
                    description = excluded.description,
                    secret_backend = excluded.secret_backend,
                    secret_account = excluded.secret_account,
                    secret_present = excluded.secret_present,
                    updated_at_ms = excluded.updated_at_ms,
                    last_rotated_at_ms = excluded.last_rotated_at_ms",
                params![
                    id,
                    provider_id,
                    label,
                    description,
                    secret_backend,
                    secret_account,
                    if has_secret { 1 } else { 0 },
                    created_at_ms,
                    now,
                    last_rotated_at_ms,
                ],
            )
            .map_err(|error| format!("Failed to save secret credential: {}", error))?;

        self.insert_audit_event(
            &connection,
            if existing.is_some() {
                "credentialUpdated"
            } else {
                "credentialCreated"
            },
            "credential",
            &id,
            &label,
        )?;

        self.get_credential(&connection, &id)?
            .map(StoredCredentialRecord::into_public)
            .ok_or_else(|| String::from("Failed to reload saved secret credential."))
    }

    fn delete_credential(
        &self,
        store: &dyn SecretStore,
        credential_id: &str,
    ) -> Result<(), String> {
        let connection = self.connect()?;
        let existing = self
            .get_credential(&connection, credential_id)?
            .ok_or_else(|| format!("Secret credential {} does not exist.", credential_id))?;

        if existing.has_secret && !existing.secret_account.is_empty() {
            let _ = store.delete_secret(&existing.secret_account);
        }

        connection
            .execute(
                "DELETE FROM secret_credentials WHERE id = ?1",
                [credential_id],
            )
            .map_err(|error| {
                format!(
                    "Failed to delete secret credential {}: {}",
                    credential_id, error
                )
            })?;

        self.insert_audit_event(
            &connection,
            "credentialDeleted",
            "credential",
            credential_id,
            &existing.label,
        )?;
        Ok(())
    }

    fn upsert_assignment(
        &self,
        input: SecretAssignmentInput,
    ) -> Result<SecretAssignmentRecord, String> {
        let connection = self.connect()?;
        let credential_id = input.credential_id.trim().to_string();

        if credential_id.is_empty() {
            return Err(String::from(
                "Secret assignment credentialId cannot be empty.",
            ));
        }
        if self.get_credential(&connection, &credential_id)?.is_none() {
            return Err(format!(
                "Secret credential {} does not exist.",
                credential_id
            ));
        }

        let target_id = input.target_id.trim().to_string();

        if target_id.is_empty() {
            return Err(String::from("Secret assignment targetId cannot be empty."));
        }

        let id = generated_id("assignment", &target_id, &input.id);
        let env_var = input.env_var.trim().to_string();
        let file_path = input.file_path.trim().to_string();

        if env_var.is_empty() && file_path.is_empty() {
            return Err(String::from(
                "Secret assignment must declare envVar or filePath.",
            ));
        }

        let existing = connection
            .query_row(
                "SELECT created_at_ms FROM secret_assignments WHERE id = ?1",
                [id.as_str()],
                |row| row.get::<_, u64>(0),
            )
            .optional()
            .map_err(|error| format!("Failed to inspect secret assignment {}: {}", id, error))?;
        let now = now_ms();
        let created_at_ms = existing.unwrap_or(now);

        connection
            .execute(
                "INSERT INTO secret_assignments (
                    id, credential_id, target_kind, target_id, env_var, file_path, enabled,
                    created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                    credential_id = excluded.credential_id,
                    target_kind = excluded.target_kind,
                    target_id = excluded.target_id,
                    env_var = excluded.env_var,
                    file_path = excluded.file_path,
                    enabled = excluded.enabled,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    id,
                    credential_id,
                    format_target_kind(input.target_kind),
                    target_id,
                    env_var,
                    file_path,
                    if input.enabled { 1 } else { 0 },
                    created_at_ms,
                    now,
                ],
            )
            .map_err(|error| format!("Failed to save secret assignment: {}", error))?;

        self.insert_audit_event(
            &connection,
            if existing.is_some() {
                "assignmentUpdated"
            } else {
                "assignmentCreated"
            },
            "assignment",
            &id,
            "",
        )?;

        self.list_assignments(&connection)?
            .into_iter()
            .find(|assignment| assignment.id == id)
            .ok_or_else(|| String::from("Failed to reload saved secret assignment."))
    }

    fn delete_assignment(&self, assignment_id: &str) -> Result<(), String> {
        let connection = self.connect()?;
        let deleted = connection
            .execute(
                "DELETE FROM secret_assignments WHERE id = ?1",
                [assignment_id],
            )
            .map_err(|error| {
                format!(
                    "Failed to delete secret assignment {}: {}",
                    assignment_id, error
                )
            })?;

        if deleted == 0 {
            return Err(format!(
                "Secret assignment {} does not exist.",
                assignment_id
            ));
        }

        self.insert_audit_event(
            &connection,
            "assignmentDeleted",
            "assignment",
            assignment_id,
            "",
        )?;
        Ok(())
    }
}

impl SqliteSecretRepository {
    fn insert_audit_event(
        &self,
        connection: &Connection,
        event_kind: &str,
        entity_kind: &str,
        entity_id: &str,
        detail: &str,
    ) -> Result<(), String> {
        connection
            .execute(
                "INSERT INTO secret_audit_events (event_kind, entity_kind, entity_id, detail, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![event_kind, entity_kind, entity_id, detail, now_ms()],
            )
            .map_err(|error| format!("Failed to write secret audit event: {}", error))?;
        Ok(())
    }
}
