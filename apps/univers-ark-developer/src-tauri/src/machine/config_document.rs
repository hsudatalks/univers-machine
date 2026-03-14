use super::{
    repository::{read_targets_config_document, save_targets_config_document},
};

pub(crate) fn read_targets_config() -> Result<String, String> {
    read_targets_config_document()
}

pub(crate) fn save_targets_config(content: &str) -> Result<(), String> {
    save_targets_config_document(content)
}
