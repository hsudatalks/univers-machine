use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct HookEvent {
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub hook_event_name: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
}

impl HookEvent {
    pub fn event_name(&self) -> &str {
        self.hook_event_name
            .as_deref()
            .or(self.event.as_deref())
            .unwrap_or("Unknown")
    }

    pub fn status(&self) -> &str {
        match self.event_name() {
            "SessionStart" | "PreToolUse" | "PostToolUse" => "active",
            "Notification" => "waiting_input",
            "Stop" => "idle",
            "SessionEnd" => "ended",
            _ => "active",
        }
    }

    pub fn tool_input_summary(&self) -> Option<String> {
        let input = self.tool_input.as_ref()?;
        let s = serde_json::to_string(input).ok()?;
        if s.len() > 200 {
            Some(format!("{}…", &s[..200]))
        } else {
            Some(s)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub cwd: String,
    pub status: String,
    pub last_event: Option<String>,
    pub last_tool: Option<String>,
    pub started_at: String,
    pub updated_at: String,
}
