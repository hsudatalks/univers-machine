use keyring::Entry;

pub(super) trait SecretStore: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn set_secret(&self, account: &str, value: &str) -> Result<(), String>;
    fn get_secret(&self, account: &str) -> Result<String, String>;
    fn delete_secret(&self, account: &str) -> Result<(), String>;
}

pub(super) struct KeyringSecretStore {
    service_name: String,
}

impl KeyringSecretStore {
    pub(super) fn new() -> Self {
        Self {
            service_name: if cfg!(debug_assertions) {
                String::from("univers-ark-developer.dev")
            } else {
                String::from("univers-ark-developer")
            },
        }
    }

    fn entry(&self, account: &str) -> Result<Entry, String> {
        Entry::new(&self.service_name, account)
            .map_err(|error| format!("Failed to access OS credential store: {}", error))
    }
}

impl SecretStore for KeyringSecretStore {
    fn backend_name(&self) -> &'static str {
        "keyring"
    }

    fn set_secret(&self, account: &str, value: &str) -> Result<(), String> {
        self.entry(account)?
            .set_password(value)
            .map_err(|error| format!("Failed to write secret to OS credential store: {}", error))
    }

    fn get_secret(&self, account: &str) -> Result<String, String> {
        self.entry(account)?
            .get_password()
            .map_err(|error| format!("Failed to read secret from OS credential store: {}", error))
    }

    fn delete_secret(&self, account: &str) -> Result<(), String> {
        self.entry(account)?.delete_credential().map_err(|error| {
            format!(
                "Failed to delete secret from OS credential store: {}",
                error
            )
        })
    }
}
