use super::{
    fs_store::{read_targets_file_content, sanitize_targets_json_content},
    repository::save_raw_targets_file,
    RawTargetsFile,
};

pub(crate) fn read_targets_config() -> Result<String, String> {
    let content = read_targets_file_content()?;
    sanitize_targets_json_content(&content)
}

pub(crate) fn save_targets_config(content: &str) -> Result<(), String> {
    let sanitized_content = sanitize_targets_json_content(content)?;
    let parsed = serde_json::from_str::<RawTargetsFile>(&sanitized_content)
        .map_err(|error| format!("Invalid config JSON: {}", error))?;

    save_raw_targets_file(&parsed)
}
