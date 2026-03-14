use crate::machine::{MachineInfo, NetworkInterface};

#[derive(Debug, Clone, Default)]
pub(crate) struct MachineApplicationService;

impl MachineApplicationService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn info(&self) -> MachineInfo {
        MachineInfo::collect()
    }

    pub(crate) fn network_interfaces(&self) -> Vec<NetworkInterface> {
        NetworkInterface::list()
    }
}
