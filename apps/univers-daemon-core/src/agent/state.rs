use crate::agent::db::Db;
use crate::agent::event::{HookEvent, SessionSnapshot};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

pub struct AgentState {
    pub sessions: RwLock<HashMap<String, SessionSnapshot>>,
}

impl AgentState {
    pub fn new() -> Arc<Self> {
        let mut map = HashMap::new();

        // Hydrate non-ended sessions from SQLite for crash recovery
        match Db::open() {
            Ok(db) => match db.list_sessions(false) {
                Ok(rows) => {
                    for row in rows {
                        let snap: SessionSnapshot = row.into();
                        map.insert(snap.session_id.clone(), snap);
                    }
                    info!("Hydrated {} active sessions from SQLite", map.len());
                }
                Err(e) => error!("Failed to hydrate sessions: {e}"),
            },
            Err(e) => error!("Failed to open DB for hydration: {e}"),
        }

        Arc::new(Self {
            sessions: RwLock::new(map),
        })
    }

    pub async fn process_event(&self, ev: HookEvent) {
        let now = Utc::now().to_rfc3339();
        let cwd = ev.cwd.clone().unwrap_or_else(|| "unknown".to_string());
        let event_name = ev.event_name().to_owned();
        let status = ev.status().to_owned();
        let tool_name = ev.tool_name.clone();
        let tool_input = ev.tool_input_summary();
        let session_id = ev.session_id.clone();

        // Update in-memory state
        {
            let mut sessions = self.sessions.write().await;
            let snap = sessions
                .entry(session_id.clone())
                .or_insert_with(|| SessionSnapshot {
                    session_id: session_id.clone(),
                    cwd: cwd.clone(),
                    status: status.clone(),
                    last_event: Some(event_name.clone()),
                    last_tool: tool_name.clone(),
                    started_at: now.clone(),
                    updated_at: now.clone(),
                });
            snap.status.clone_from(&status);
            snap.last_event = Some(event_name.clone());
            if tool_name.is_some() {
                snap.last_tool.clone_from(&tool_name);
            }
            snap.updated_at = now;
        }

        // Async persist to SQLite
        tokio::task::spawn_blocking(move || {
            let Ok(db) = Db::open() else {
                error!("Failed to open DB for persist");
                return;
            };
            if let Err(e) = db.upsert_session(
                &session_id,
                &cwd,
                &status,
                &event_name,
                tool_name.as_deref(),
            ) {
                error!("Failed to persist session: {e}");
            }
            if let Err(e) = db.insert_event(
                &ev.session_id,
                &event_name,
                tool_name.as_deref(),
                tool_input.as_deref(),
            ) {
                error!("Failed to persist event: {e}");
            }
        });
    }

    pub async fn list_sessions(&self, include_ended: bool) -> Vec<SessionSnapshot> {
        let sessions = self.sessions.read().await;
        let in_memory: HashMap<String, SessionSnapshot> = sessions
            .iter()
            .map(|(session_id, snapshot)| (session_id.clone(), snapshot.clone()))
            .collect();
        drop(sessions);

        if !include_ended {
            let mut result: Vec<SessionSnapshot> = in_memory
                .into_values()
                .filter(|s| s.status != "ended")
                .collect();
            result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            return result;
        }

        let mut merged = match tokio::task::spawn_blocking(|| {
            let db = Db::open().map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows = db.list_sessions(true).map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok::<_, anyhow::Error>(
                rows.into_iter()
                    .map(SessionSnapshot::from)
                    .map(|snapshot| (snapshot.session_id.clone(), snapshot))
                    .collect::<HashMap<_, _>>(),
            )
        })
        .await
        {
            Ok(Ok(rows)) => rows,
            Ok(Err(e)) => {
                error!("Failed to load historical sessions from SQLite: {e}");
                HashMap::new()
            }
            Err(e) => {
                error!("Failed to join historical session load task: {e}");
                HashMap::new()
            }
        };

        for (session_id, snapshot) in in_memory {
            merged.insert(session_id, snapshot);
        }

        let mut result: Vec<SessionSnapshot> = merged.into_values().collect();
        result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        result
    }
}
