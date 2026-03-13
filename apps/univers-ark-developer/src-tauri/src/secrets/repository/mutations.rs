use super::{
    helpers::{format_target_kind, generated_id, now_ms, StoredCredentialRecord},
    SqliteSecretRepository,
};
use crate::{
    models::{
        SecretAssignmentInput, SecretAssignmentRecord, SecretCredentialInput,
        SecretCredentialRecord, SecretProviderInput, SecretProviderRecord,
    },
    secrets::store::SecretStore,
};
use rusqlite::{params, Connection, OptionalExtension};

impl SqliteSecretRepository {
    pub(super) fn upsert_provider(
        &self,
        input: SecretProviderInput,
    ) -> Result<SecretProviderRecord, String> {
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
            .map_err(|error| format!("Failed to save secret provider: {error}"))?;

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

    pub(super) fn delete_provider(&self, provider_id: &str) -> Result<(), String> {
        let connection = self.connect()?;
        let deleted = connection
            .execute("DELETE FROM secret_providers WHERE id = ?1", [provider_id])
            .map_err(|error| {
                format!(
                    "Failed to delete secret provider {provider_id}: {error}"
                )
            })?;

        if deleted == 0 {
            return Err(format!("Secret provider {provider_id} does not exist."));
        }

        self.insert_audit_event(&connection, "providerDeleted", "provider", provider_id, "")?;
        Ok(())
    }

    pub(super) fn upsert_credential(
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
            return Err(format!("Secret provider {provider_id} does not exist."));
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
            .map_err(|error| format!("Failed to save secret credential: {error}"))?;

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

    pub(super) fn delete_credential(
        &self,
        store: &dyn SecretStore,
        credential_id: &str,
    ) -> Result<(), String> {
        let connection = self.connect()?;
        let existing = self
            .get_credential(&connection, credential_id)?
            .ok_or_else(|| format!("Secret credential {credential_id} does not exist."))?;

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
                    "Failed to delete secret credential {credential_id}: {error}"
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

    pub(super) fn resolve_credential_secret_value(
        &self,
        store: &dyn SecretStore,
        credential_id: &str,
    ) -> Result<String, String> {
        let connection = self.connect()?;
        let existing = self
            .get_credential(&connection, credential_id)?
            .ok_or_else(|| format!("Secret credential {credential_id} does not exist."))?;

        if !existing.has_secret || existing.secret_account.trim().is_empty() {
            return Err(format!(
                "Secret credential {credential_id} does not have a stored secret."
            ));
        }

        store.get_secret(&existing.secret_account)
    }

    pub(super) fn upsert_assignment(
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
                "Secret credential {credential_id} does not exist."
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
            .map_err(|error| format!("Failed to inspect secret assignment {id}: {error}"))?;
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
            .map_err(|error| format!("Failed to save secret assignment: {error}"))?;

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

    pub(super) fn delete_assignment(&self, assignment_id: &str) -> Result<(), String> {
        let connection = self.connect()?;
        let deleted = connection
            .execute(
                "DELETE FROM secret_assignments WHERE id = ?1",
                [assignment_id],
            )
            .map_err(|error| {
                format!(
                    "Failed to delete secret assignment {assignment_id}: {error}"
                )
            })?;

        if deleted == 0 {
            return Err(format!(
                "Secret assignment {assignment_id} does not exist."
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

    pub(super) fn insert_audit_event(
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
            .map_err(|error| format!("Failed to write secret audit event: {error}"))?;
        Ok(())
    }
}
