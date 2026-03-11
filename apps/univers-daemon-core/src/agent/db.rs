use crate::agent::event::SessionSnapshot;
use chrono::Utc;
use rusqlite::{params, Connection, Result};
use std::path::PathBuf;

pub struct Db {
    conn: Connection,
}

pub struct SessionRow {
    pub session_id: String,
    pub cwd: String,
    pub status: String,
    pub last_event: Option<String>,
    pub last_tool: Option<String>,
    pub started_at: String,
    pub updated_at: String,
}

impl From<SessionRow> for SessionSnapshot {
    fn from(row: SessionRow) -> Self {
        Self {
            session_id: row.session_id,
            cwd: row.cwd,
            status: row.status,
            last_event: row.last_event,
            last_tool: row.last_tool,
            started_at: row.started_at,
            updated_at: row.updated_at,
        }
    }
}

impl Db {
    pub fn open() -> Result<Self> {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                cwd        TEXT NOT NULL,
                status     TEXT NOT NULL DEFAULT 'active',
                last_event TEXT,
                last_tool  TEXT,
                started_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                event_name TEXT NOT NULL,
                tool_name  TEXT,
                tool_input TEXT,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn upsert_session(
        &self,
        session_id: &str,
        cwd: &str,
        status: &str,
        event_name: &str,
        tool_name: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO sessions (session_id, cwd, status, last_event, last_tool, started_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(session_id) DO UPDATE SET
                 status = ?3,
                 last_event = ?4,
                 last_tool = COALESCE(?5, last_tool),
                 updated_at = ?6",
            params![session_id, cwd, status, event_name, tool_name, now],
        )?;
        Ok(())
    }

    pub fn insert_event(
        &self,
        session_id: &str,
        event_name: &str,
        tool_name: Option<&str>,
        tool_input: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO events (session_id, event_name, tool_name, tool_input, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, event_name, tool_name, tool_input, now],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self, include_ended: bool) -> Result<Vec<SessionRow>> {
        let sql = if include_ended {
            "SELECT session_id, cwd, status, last_event, last_tool, started_at, updated_at
             FROM sessions ORDER BY updated_at DESC"
        } else {
            "SELECT session_id, cwd, status, last_event, last_tool, started_at, updated_at
             FROM sessions WHERE status != 'ended' ORDER BY updated_at DESC"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(SessionRow {
                session_id: row.get(0)?,
                cwd: row.get(1)?,
                status: row.get(2)?,
                last_event: row.get(3)?,
                last_tool: row.get(4)?,
                started_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn clean_old(&self, hours: u32) -> Result<usize> {
        let cutoff = (Utc::now() - chrono::Duration::hours(i64::from(hours))).to_rfc3339();
        self.conn.execute(
            "DELETE FROM events WHERE session_id IN
             (SELECT session_id FROM sessions WHERE status = 'ended' AND updated_at < ?1)",
            params![cutoff],
        )?;
        let deleted = self.conn.execute(
            "DELETE FROM sessions WHERE status = 'ended' AND updated_at < ?1",
            params![cutoff],
        )?;
        Ok(deleted)
    }
}

fn db_path() -> PathBuf {
    dirs_home().join(".claude").join("monitor.db")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}
