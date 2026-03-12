use super::{MachineTransport, ManagedContainerKind, ManagedServer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BrowserServiceType {
    #[default]
    Http,
    Vite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DeveloperServiceKind {
    #[serde(alias = "browser")]
    #[default]
    Web,
    Endpoint,
    Command,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EndpointProbeType {
    #[default]
    Http,
    Tcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserSurface {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) service_type: BrowserServiceType,
    #[serde(default)]
    pub(crate) background_prerender: bool,
    pub(crate) tunnel_command: String,
    pub(crate) local_url: String,
    pub(crate) remote_url: String,
    #[serde(default)]
    pub(crate) vite_hmr_tunnel_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeveloperService {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) kind: DeveloperServiceKind,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    #[serde(alias = "browser")]
    pub(crate) web: Option<BrowserSurface>,
    #[serde(default)]
    pub(crate) endpoint: Option<EndpointService>,
    #[serde(default)]
    pub(crate) command: Option<CommandService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandService {
    #[serde(default)]
    pub(crate) restart: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerWorkspace {
    #[serde(default)]
    pub(crate) profile: String,
    #[serde(default)]
    pub(crate) default_tool: String,
    #[serde(default)]
    pub(crate) project_path: String,
    #[serde(default)]
    pub(crate) files_root: String,
    #[serde(default)]
    #[serde(alias = "primaryBrowserServiceId")]
    pub(crate) primary_web_service_id: String,
    #[serde(default)]
    pub(crate) tmux_command_service_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EndpointService {
    #[serde(default)]
    pub(crate) probe_type: EndpointProbeType,
    #[serde(default)]
    pub(crate) host: String,
    pub(crate) port: u16,
    #[serde(default)]
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeveloperTarget {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) machine_id: String,
    #[serde(default)]
    pub(crate) container_id: String,
    #[serde(default)]
    pub(crate) transport: MachineTransport,
    #[serde(default)]
    pub(crate) container_kind: ManagedContainerKind,
    pub(crate) label: String,
    pub(crate) host: String,
    pub(crate) description: String,
    pub(crate) terminal_command: String,
    #[serde(default)]
    pub(crate) terminal_startup_command: String,
    #[serde(default)]
    pub(crate) notes: Vec<String>,
    #[serde(default)]
    pub(crate) workspace: ContainerWorkspace,
    #[serde(default)]
    pub(crate) services: Vec<DeveloperService>,
    #[serde(default)]
    pub(crate) surfaces: Vec<BrowserSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TargetsFile {
    pub(crate) selected_target_id: Option<String>,
    pub(crate) default_profile: Option<String>,
    pub(crate) targets: Vec<DeveloperTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) theme_mode: String,
    pub(crate) dashboard_refresh_seconds: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: String::from("system"),
            dashboard_refresh_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppBootstrap {
    pub(crate) app_name: String,
    pub(crate) config_path: String,
    pub(crate) selected_target_id: Option<String>,
    pub(crate) targets: Vec<DeveloperTarget>,
    pub(crate) machines: Vec<ManagedServer>,
}
