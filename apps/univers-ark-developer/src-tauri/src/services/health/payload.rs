use crate::models::ContainerServiceInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardServicePayload {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) url: Option<String>,
}

pub(crate) fn into_container_service_infos(
    payloads: Vec<DashboardServicePayload>,
) -> Vec<ContainerServiceInfo> {
    payloads
        .into_iter()
        .map(|service| ContainerServiceInfo {
            id: service.id,
            label: service.label,
            status: service.status,
            detail: service.detail,
            url: service.url,
        })
        .collect()
}
