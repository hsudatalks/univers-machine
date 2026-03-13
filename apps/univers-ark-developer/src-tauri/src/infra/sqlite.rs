use rusqlite::Connection;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct SqliteStore {
    path: PathBuf,
}

impl SqliteStore {
    pub(crate) fn new(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create SQLite directory {}: {}",
                    parent.display(),
                    error
                )
            })?;
        }

        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path).map_err(|error| {
            format!(
                "Failed to open SQLite database {}: {}",
                self.path.display(),
                error
            )
        })?;

        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA busy_timeout=5000;
                 PRAGMA foreign_keys=ON;",
            )
            .map_err(|error| format!("Failed to initialize SQLite pragmas: {error}"))?;

        Ok(connection)
    }

    pub(crate) fn migrate(&self, schema: &str) -> Result<(), String> {
        self.connect()?
            .execute_batch(schema)
            .map_err(|error| format!("Failed to migrate SQLite schema: {error}"))
    }
}
