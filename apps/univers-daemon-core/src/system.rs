use serde::Serialize;
use sysinfo::System;

use crate::sysdetect::EnvironmentKind;

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub kernel: String,
    pub uptime_secs: u64,
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub memory_percent: f32,
    pub cpu_count: usize,
    pub cpu_usage_percent: f32,
    pub disk_total_gb: f64,
    pub disk_used_gb: f64,
    pub disk_percent: f32,
    pub environment: EnvironmentKind,
    pub daemon_version: String,
    pub collected_at: String,
}

impl SystemInfo {
    pub fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = System::host_name().unwrap_or_else(|| "unknown".into());
        let os = System::name().unwrap_or_else(|| "unknown".into());
        let os_version = System::os_version().unwrap_or_else(|| "unknown".into());
        let kernel = System::kernel_version().unwrap_or_else(|| "unknown".into());
        let arch = std::env::consts::ARCH.to_string();
        let uptime_secs = System::uptime();

        let memory_total_mb = sys.total_memory() / 1024 / 1024;
        let memory_used_mb = sys.used_memory() / 1024 / 1024;
        let memory_percent = if memory_total_mb > 0 {
            (memory_used_mb as f32 / memory_total_mb as f32) * 100.0
        } else {
            0.0
        };

        let cpu_count = sys.cpus().len();
        let cpu_usage_percent = sys.global_cpu_info().cpu_usage();

        let disks = sysinfo::Disks::new_with_refreshed_list();
        let (disk_total, disk_used) = disks.iter().fold((0u64, 0u64), |(t, u), d| {
            (t + d.total_space(), u + (d.total_space() - d.available_space()))
        });
        let disk_total_gb = disk_total as f64 / 1_073_741_824.0;
        let disk_used_gb = disk_used as f64 / 1_073_741_824.0;
        let disk_percent = if disk_total > 0 {
            (disk_used as f32 / disk_total as f32) * 100.0
        } else {
            0.0
        };

        let environment = EnvironmentKind::detect();
        let collected_at = chrono::Utc::now().to_rfc3339();

        Self {
            hostname,
            os,
            os_version,
            arch,
            kernel,
            uptime_secs,
            memory_total_mb,
            memory_used_mb,
            memory_percent,
            cpu_count,
            cpu_usage_percent,
            disk_total_gb,
            disk_used_gb,
            disk_percent,
            environment,
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            collected_at,
        }
    }
}
