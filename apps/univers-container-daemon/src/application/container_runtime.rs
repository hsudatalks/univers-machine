use crate::container::{
    collect_ports, ContainerInfo, ContainerPortInfo, ContainerProcessesInfo, ContainerRuntimeInfo,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ContainerRuntimeApplicationService;

impl ContainerRuntimeApplicationService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn info(&self) -> ContainerInfo {
        ContainerInfo::collect()
    }

    pub(crate) fn runtime(&self) -> ContainerRuntimeInfo {
        ContainerRuntimeInfo::collect()
    }

    pub(crate) fn processes(&self) -> ContainerProcessesInfo {
        ContainerProcessesInfo::collect()
    }

    pub(crate) fn ports(&self) -> Vec<ContainerPortInfo> {
        collect_ports()
    }
}
