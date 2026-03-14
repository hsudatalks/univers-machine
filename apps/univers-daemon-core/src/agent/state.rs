use crate::agent::{
    event::{HookEvent, SessionSnapshot},
    projector::SessionProjector,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AgentState {
    pub sessions: RwLock<HashMap<String, SessionSnapshot>>,
}

impl AgentState {
    pub fn new(initial_sessions: Vec<SessionSnapshot>) -> Arc<Self> {
        Arc::new(Self {
            sessions: RwLock::new(
                initial_sessions
                    .into_iter()
                    .map(|snapshot| (snapshot.session_id.clone(), snapshot))
                    .collect(),
            ),
        })
    }

    pub async fn apply_event(&self, ev: &HookEvent) -> SessionSnapshot {
        let now = Utc::now().to_rfc3339();
        let session_id = ev.session_id.clone();

        let mut sessions = self.sessions.write().await;
        let next_snapshot = SessionProjector::apply(sessions.get(&session_id), ev, &now);
        sessions.insert(session_id, next_snapshot.clone());
        next_snapshot
    }

    pub async fn active_sessions(&self) -> Vec<SessionSnapshot> {
        let sessions = self.sessions.read().await;
        let mut result: Vec<SessionSnapshot> = sessions.values().cloned().collect();
        result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        result
    }
}
