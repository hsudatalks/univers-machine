use crate::agent::{
    event::{HookEvent, SessionSnapshot},
    repository::SessionRepository,
    state::AgentState,
};
use std::sync::Arc;
use tracing::{error, info};

pub struct AgentSessionApplicationService {
    state: Arc<AgentState>,
    repository: Arc<dyn SessionRepository>,
}

impl AgentSessionApplicationService {
    pub fn new(repository: Arc<dyn SessionRepository>) -> Self {
        let state = Self::hydrate_state(repository.as_ref());
        Self { state, repository }
    }

    pub async fn process_event(&self, event: HookEvent) {
        self.state.apply_event(&event).await;

        let repository = self.repository.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = repository.persist_event(&event) {
                error!("Failed to persist session event: {error}");
            }
        });
    }

    pub async fn list_sessions(&self, include_ended: bool) -> Vec<SessionSnapshot> {
        let active_sessions = self.state.active_sessions().await;
        if !include_ended {
            return active_sessions
                .into_iter()
                .filter(|session| session.status != "ended")
                .collect();
        }

        let repository = self.repository.clone();
        let mut merged = match tokio::task::spawn_blocking(move || repository.list_sessions(true)).await
        {
            Ok(Ok(sessions)) => sessions
                .into_iter()
                .map(|snapshot| (snapshot.session_id.clone(), snapshot))
                .collect(),
            Ok(Err(error)) => {
                error!("Failed to load historical sessions from session repository: {error}");
                std::collections::HashMap::new()
            }
            Err(error) => {
                error!("Failed to join historical session load task: {error}");
                std::collections::HashMap::new()
            }
        };

        for session in active_sessions {
            merged.insert(session.session_id.clone(), session);
        }

        let mut sessions: Vec<SessionSnapshot> = merged.into_values().collect();
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }

    pub async fn clean_old_sessions(&self, hours: u32) -> anyhow::Result<usize> {
        let repository = self.repository.clone();
        tokio::task::spawn_blocking(move || repository.clean_old(hours)).await?
    }

    fn hydrate_state(repository: &dyn SessionRepository) -> Arc<AgentState> {
        match repository.list_sessions(false) {
            Ok(sessions) => {
                info!(
                    "Hydrated {} active sessions from session repository",
                    sessions.len()
                );
                AgentState::new(sessions)
            }
            Err(error) => {
                error!("Failed to hydrate sessions from session repository: {error}");
                AgentState::new(Vec::new())
            }
        }
    }
}
