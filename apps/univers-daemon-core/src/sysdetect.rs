use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum EnvironmentKind {
    DockerContainer,
    OrbstackContainer,
    Wsl,
    VirtualMachine,
    BareMetal,
    Unknown,
}

impl EnvironmentKind {
    pub fn detect() -> Self {
        // Check OrbStack first (it's also Docker but more specific)
        if is_orbstack() {
            return Self::OrbstackContainer;
        }

        // Docker / container detection
        if is_docker() {
            return Self::DockerContainer;
        }

        // WSL detection
        if is_wsl() {
            return Self::Wsl;
        }

        // VM detection
        if is_virtual_machine() {
            return Self::VirtualMachine;
        }

        // If no container/VM indicators found, assume bare metal
        if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
            return Self::BareMetal;
        }

        Self::Unknown
    }
}

fn is_orbstack() -> bool {
    // OrbStack sets specific env vars or has identifiable kernel strings
    if std::env::var("ORBSTACK").is_ok() {
        return true;
    }
    // OrbStack containers have "orbstack" in kernel version
    if let Some(kernel) = sysinfo::System::kernel_version() {
        if kernel.to_lowercase().contains("orbstack") {
            return true;
        }
    }
    false
}

fn is_docker() -> bool {
    // /.dockerenv exists in Docker containers
    if Path::new("/.dockerenv").exists() {
        return true;
    }
    // Check cgroup for docker/containerd
    if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
        if cgroup.contains("docker") || cgroup.contains("containerd") {
            return true;
        }
    }
    // container env var
    if let Ok(val) = std::env::var("container") {
        if val == "docker" {
            return true;
        }
    }
    false
}

fn is_wsl() -> bool {
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        if version.to_lowercase().contains("microsoft")
            || version.to_lowercase().contains("wsl")
        {
            return true;
        }
    }
    std::env::var("WSL_DISTRO_NAME").is_ok()
}

fn is_virtual_machine() -> bool {
    // Check DMI product name for common hypervisors
    if let Ok(product) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
        let product_lower = product.to_lowercase();
        if product_lower.contains("virtualbox")
            || product_lower.contains("vmware")
            || product_lower.contains("kvm")
            || product_lower.contains("qemu")
            || product_lower.contains("hyper-v")
            || product_lower.contains("xen")
        {
            return true;
        }
    }
    // Check systemd-detect-virt style checks
    if let Ok(hypervisor) = std::fs::read_to_string("/sys/hypervisor/type") {
        if !hypervisor.trim().is_empty() {
            return true;
        }
    }
    false
}
