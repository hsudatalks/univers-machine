use crate::{
    models::{
        SecretAssignmentInput, SecretAssignmentRecord, SecretCredentialInput,
        SecretCredentialRecord, SecretInventory, SecretProviderInput, SecretProviderRecord,
    },
    secrets::SecretManagementState,
};
use tauri::State;

#[tauri::command]
pub(crate) fn load_secret_inventory(
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<SecretInventory, String> {
    secret_management_state.load_inventory()
}

#[tauri::command]
pub(crate) fn upsert_secret_provider(
    input: SecretProviderInput,
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<SecretProviderRecord, String> {
    secret_management_state.upsert_provider(input)
}

#[tauri::command]
pub(crate) fn delete_secret_provider(
    provider_id: String,
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<(), String> {
    secret_management_state.delete_provider(&provider_id)
}

#[tauri::command]
pub(crate) fn upsert_secret_credential(
    input: SecretCredentialInput,
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<SecretCredentialRecord, String> {
    secret_management_state.upsert_credential(input)
}

#[tauri::command]
pub(crate) fn delete_secret_credential(
    credential_id: String,
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<(), String> {
    secret_management_state.delete_credential(&credential_id)
}

#[tauri::command]
pub(crate) fn upsert_secret_assignment(
    input: SecretAssignmentInput,
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<SecretAssignmentRecord, String> {
    secret_management_state.upsert_assignment(input)
}

#[tauri::command]
pub(crate) fn delete_secret_assignment(
    assignment_id: String,
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<(), String> {
    secret_management_state.delete_assignment(&assignment_id)
}
