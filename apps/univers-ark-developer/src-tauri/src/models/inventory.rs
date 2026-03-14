use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MachineTransport {
    Local,
    #[default]
    Ssh,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ManagedContainerKind {
    Host,
    #[default]
    Managed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedContainer {
    pub(crate) server_id: String,
    pub(crate) server_label: String,
    pub(crate) container_id: String,
    pub(crate) kind: ManagedContainerKind,
    pub(crate) transport: MachineTransport,
    pub(crate) target_id: String,
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) ipv4: String,
    pub(crate) ssh_user: String,
    pub(crate) ssh_destination: String,
    pub(crate) ssh_command: String,
    pub(crate) ssh_state: String,
    pub(crate) ssh_message: String,
    pub(crate) ssh_reachable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedServer {
    pub(crate) id: String,
    pub(crate) host_target_id: String,
    pub(crate) label: String,
    pub(crate) transport: MachineTransport,
    pub(crate) host: String,
    pub(crate) description: String,
    pub(crate) os: String,
    pub(crate) state: String,
    pub(crate) message: String,
    pub(crate) containers: Vec<ManagedContainer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportedMachineJump {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) user: String,
    pub(crate) identity_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MachineImportCandidate {
    pub(crate) import_id: String,
    pub(crate) machine_id: String,
    pub(crate) label: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) ssh_user: String,
    pub(crate) identity_files: Vec<String>,
    pub(crate) jump_chain: Vec<ImportedMachineJump>,
    pub(crate) description: String,
    pub(crate) detail: String,
}
