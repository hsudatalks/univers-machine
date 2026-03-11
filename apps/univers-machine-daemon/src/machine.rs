use serde::Serialize;
use sysinfo::{Networks, System};

/// Hardware and system information specific to physical/virtual machines.
#[derive(Debug, Clone, Serialize)]
pub struct MachineInfo {
    pub hostname: String,
    pub cpu_brand: String,
    pub cpu_count: usize,
    pub total_memory_gb: f64,
    pub gpu: Option<String>,
    pub network_interfaces: Vec<NetworkInterface>,
    pub disk_details: Vec<DiskDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInterface {
    pub name: String,
    pub mac_address: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskDetail {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_gb: f64,
    pub available_gb: f64,
    pub is_removable: bool,
}

impl MachineInfo {
    pub fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = System::host_name().unwrap_or_else(|| "unknown".into());
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "unknown".into());
        let cpu_count = sys.cpus().len();
        let total_memory_gb = sys.total_memory() as f64 / 1_073_741_824.0;

        let gpu = detect_gpu();

        let networks = Networks::new_with_refreshed_list();
        let network_interfaces: Vec<NetworkInterface> = networks
            .iter()
            .map(|(name, data)| NetworkInterface {
                name: name.clone(),
                mac_address: data.mac_address().to_string(),
                received_bytes: data.total_received(),
                transmitted_bytes: data.total_transmitted(),
            })
            .collect();

        let disks = sysinfo::Disks::new_with_refreshed_list();
        let disk_details: Vec<DiskDetail> = disks
            .iter()
            .map(|d| DiskDetail {
                name: d.name().to_string_lossy().to_string(),
                mount_point: d.mount_point().to_string_lossy().to_string(),
                file_system: d.file_system().to_string_lossy().to_string(),
                total_gb: d.total_space() as f64 / 1_073_741_824.0,
                available_gb: d.available_space() as f64 / 1_073_741_824.0,
                is_removable: d.is_removable(),
            })
            .collect();

        Self {
            hostname,
            cpu_brand,
            cpu_count,
            total_memory_gb,
            gpu,
            network_interfaces,
            disk_details,
        }
    }
}

impl NetworkInterface {
    pub fn list() -> Vec<Self> {
        let networks = Networks::new_with_refreshed_list();
        networks
            .iter()
            .map(|(name, data)| NetworkInterface {
                name: name.clone(),
                mac_address: data.mac_address().to_string(),
                received_bytes: data.total_received(),
                transmitted_bytes: data.total_transmitted(),
            })
            .collect()
    }
}

/// Try to detect GPU via common Linux paths.
fn detect_gpu() -> Option<String> {
    // Try lspci for GPU info
    if let Ok(output) = std::process::Command::new("lspci")
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("VGA") || line.contains("3D") || line.contains("Display") {
                    // Extract the device description after the type
                    if let Some(desc) = line.split(": ").nth(1) {
                        return Some(desc.trim().to_string());
                    }
                }
            }
        }
    }
    // Fallback: check /proc/driver/nvidia
    if std::path::Path::new("/proc/driver/nvidia/version").exists() {
        if let Ok(content) = std::fs::read_to_string("/proc/driver/nvidia/version") {
            return Some(content.lines().next().unwrap_or("NVIDIA GPU").to_string());
        }
    }
    None
}
