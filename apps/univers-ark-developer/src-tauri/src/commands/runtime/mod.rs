pub(crate) mod dashboard;
pub(crate) mod misc;
pub(crate) mod terminal;
pub(crate) mod tunnel;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TunnelRestartSpec {
    pub(crate) target_id: String,
    #[serde(alias = "surfaceId")]
    pub(crate) service_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandServiceActionSpec {
    pub(crate) target_id: String,
    pub(crate) service_id: String,
    pub(crate) action: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeActivityInput {
    visible: bool,
    focused: bool,
    online: bool,
    active_machine_id: Option<String>,
    active_target_id: Option<String>,
}
