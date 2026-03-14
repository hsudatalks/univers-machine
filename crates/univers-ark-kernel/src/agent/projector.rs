use crate::agent::event::{HookEvent, SessionSnapshot};

pub struct SessionProjector;

impl SessionProjector {
    pub fn apply(
        existing: Option<&SessionSnapshot>,
        event: &HookEvent,
        occurred_at: &str,
    ) -> SessionSnapshot {
        let cwd = event.cwd.clone().unwrap_or_else(|| "unknown".to_string());
        let event_name = event.event_name().to_owned();
        let status = event.status().to_owned();
        let tool_name = event.tool_name.clone();

        let mut snapshot = existing.cloned().unwrap_or_else(|| SessionSnapshot {
            session_id: event.session_id.clone(),
            cwd,
            status,
            last_event: Some(event_name),
            last_tool: tool_name,
            started_at: occurred_at.to_string(),
            updated_at: occurred_at.to_string(),
        });

        snapshot.status = event.status().to_owned();
        snapshot.cwd = event.cwd.clone().unwrap_or_else(|| snapshot.cwd.clone());
        snapshot.last_event = Some(event.event_name().to_owned());
        if let Some(tool_name) = &event.tool_name {
            snapshot.last_tool = Some(tool_name.clone());
        }
        snapshot.updated_at = occurred_at.to_string();

        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::SessionProjector;
    use crate::agent::event::{HookEvent, SessionSnapshot};

    #[test]
    fn initializes_snapshot_from_first_event() {
        let event = HookEvent {
            session_id: "session-1".into(),
            cwd: Some("/workspace/app".into()),
            hook_event_name: Some("SessionStart".into()),
            event: None,
            tool_name: None,
            tool_input: None,
        };

        let snapshot = SessionProjector::apply(None, &event, "2025-01-01T00:00:00Z");

        assert_eq!(snapshot.session_id, "session-1");
        assert_eq!(snapshot.cwd, "/workspace/app");
        assert_eq!(snapshot.status, "active");
        assert_eq!(snapshot.last_event.as_deref(), Some("SessionStart"));
        assert_eq!(snapshot.started_at, "2025-01-01T00:00:00Z");
        assert_eq!(snapshot.updated_at, "2025-01-01T00:00:00Z");
    }

    #[test]
    fn updates_existing_snapshot_without_losing_previous_tool() {
        let existing = SessionSnapshot {
            session_id: "session-1".into(),
            cwd: "/workspace/app".into(),
            status: "active".into(),
            last_event: Some("PreToolUse".into()),
            last_tool: Some("Read".into()),
            started_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:01:00Z".into(),
        };
        let event = HookEvent {
            session_id: "session-1".into(),
            cwd: None,
            hook_event_name: Some("Notification".into()),
            event: None,
            tool_name: None,
            tool_input: None,
        };

        let snapshot = SessionProjector::apply(Some(&existing), &event, "2025-01-01T00:02:00Z");

        assert_eq!(snapshot.cwd, "/workspace/app");
        assert_eq!(snapshot.status, "waiting_input");
        assert_eq!(snapshot.last_event.as_deref(), Some("Notification"));
        assert_eq!(snapshot.last_tool.as_deref(), Some("Read"));
        assert_eq!(snapshot.started_at, "2025-01-01T00:00:00Z");
        assert_eq!(snapshot.updated_at, "2025-01-01T00:02:00Z");
    }
}
