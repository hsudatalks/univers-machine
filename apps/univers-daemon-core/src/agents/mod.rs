use crate::agent::event::SessionSnapshot;
use crate::app::{AppCatalog, AppStatus};
use crate::installer::InstallerRegistry;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpec {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub required_apps: Vec<String>,
    pub skills: Vec<String>,
    pub commands: Vec<String>,
    pub default_command: String,
    pub default_args: Vec<String>,
    pub default_window_title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub required_apps: Vec<String>,
    pub skills: Vec<String>,
    pub commands: Vec<String>,
    pub default_command: String,
    pub default_args: Vec<String>,
    pub default_window_title: String,
    pub ready: bool,
    pub missing_apps: Vec<String>,
    pub active_sessions: u64,
    pub last_activity: Option<String>,
}

#[derive(Debug, Default)]
pub struct AgentCatalog;

impl AgentCatalog {
    pub fn new() -> Self {
        Self
    }

    pub fn list_specs(&self) -> Vec<AgentSpec> {
        builtin_agents()
    }

    pub async fn list_statuses(
        &self,
        apps: &AppCatalog,
        installers: &InstallerRegistry,
        sessions: &[SessionSnapshot],
    ) -> Vec<AgentStatus> {
        let app_statuses = apps.list_statuses(installers).await;
        self.list_specs()
            .into_iter()
            .map(|spec| status_for_spec(spec, &app_statuses, sessions))
            .collect()
    }

    pub async fn status_for(
        &self,
        id: &str,
        apps: &AppCatalog,
        installers: &InstallerRegistry,
        sessions: &[SessionSnapshot],
    ) -> anyhow::Result<AgentStatus> {
        let app_statuses = apps.list_statuses(installers).await;
        let spec = self
            .list_specs()
            .into_iter()
            .find(|spec| spec.id == id)
            .ok_or_else(|| anyhow::anyhow!("Unknown agent '{id}'"))?;
        Ok(status_for_spec(spec, &app_statuses, sessions))
    }
}

fn status_for_spec(
    spec: AgentSpec,
    app_statuses: &[AppStatus],
    sessions: &[SessionSnapshot],
) -> AgentStatus {
    let missing_apps = spec
        .required_apps
        .iter()
        .filter(|required| {
            !app_statuses
                .iter()
                .any(|status| &status.id == *required && status.installed)
        })
        .cloned()
        .collect::<Vec<_>>();

    let command_names = command_names_for_provider(&spec.provider);
    let matching_sessions = sessions
        .iter()
        .filter(|session| {
            session.status != "ended"
                && session
                    .last_tool
                    .as_deref()
                    .map(|tool| command_names.iter().any(|name| *name == tool))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let last_activity = matching_sessions
        .iter()
        .map(|session| session.updated_at.clone())
        .max();

    AgentStatus {
        id: spec.id,
        title: spec.title,
        provider: spec.provider,
        required_apps: spec.required_apps,
        skills: spec.skills,
        commands: spec.commands,
        default_command: spec.default_command,
        default_args: spec.default_args,
        default_window_title: spec.default_window_title,
        ready: missing_apps.is_empty(),
        missing_apps,
        active_sessions: matching_sessions.len() as u64,
        last_activity,
    }
}

fn builtin_agents() -> Vec<AgentSpec> {
    vec![
        AgentSpec {
            id: String::from("claude-code-dev"),
            title: String::from("Claude Code Dev Agent"),
            provider: String::from("claude-code"),
            required_apps: vec![String::from("claude-code")],
            skills: vec![String::from("coding"), String::from("terminal")],
            commands: vec![String::from("claude"), String::from("claude --continue")],
            default_command: String::from("claude"),
            default_args: Vec::new(),
            default_window_title: String::from("Dev"),
        },
        AgentSpec {
            id: String::from("opencode-dev"),
            title: String::from("OpenCode Dev Agent"),
            provider: String::from("opencode"),
            required_apps: vec![String::from("opencode")],
            skills: vec![String::from("coding"), String::from("terminal")],
            commands: vec![String::from("opencode")],
            default_command: String::from("opencode"),
            default_args: Vec::new(),
            default_window_title: String::from("OpenCode"),
        },
        AgentSpec {
            id: String::from("codex-cli-dev"),
            title: String::from("Codex CLI Dev Agent"),
            provider: String::from("codex-cli"),
            required_apps: vec![String::from("codex-cli")],
            skills: vec![String::from("coding"), String::from("terminal")],
            commands: vec![String::from("codex")],
            default_command: String::from("codex"),
            default_args: Vec::new(),
            default_window_title: String::from("Codex"),
        },
    ]
}

fn command_names_for_provider(provider: &str) -> &[&str] {
    match provider {
        "claude-code" => &["claude", "Claude"],
        "opencode" => &["opencode", "OpenCode"],
        "codex-cli" => &["codex", "Codex"],
        _ => &[],
    }
}
