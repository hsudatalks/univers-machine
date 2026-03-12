mod payload;
mod probe;

pub(crate) use self::{
    payload::{into_container_service_infos, DashboardServicePayload},
    probe::dashboard_probe_command,
};
