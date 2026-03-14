use serde::Serialize;
use std::{
    fs,
    process::Command,
};
use sysinfo::System;
use univers_daemon_shared::sysdetect::EnvironmentKind;

const PROCESS_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContainerInfo {
    pub(crate) container_id: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) hostname: String,
    pub(crate) mounts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerRuntimeInfo {
    pub(crate) hostname: String,
    pub(crate) uptime_seconds: u64,
    pub(crate) process_count: usize,
    pub(crate) load_average_1m: f64,
    pub(crate) load_average_5m: f64,
    pub(crate) load_average_15m: f64,
    pub(crate) memory_total_bytes: u64,
    pub(crate) memory_used_bytes: u64,
    pub(crate) disk_total_bytes: u64,
    pub(crate) disk_used_bytes: u64,
    pub(crate) environment: EnvironmentKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerProcessInfo {
    pub(crate) pid: u32,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) cpu_usage_percent: f32,
    pub(crate) memory_bytes: u64,
    pub(crate) virtual_memory_bytes: u64,
    pub(crate) command: Option<String>,
    pub(crate) executable_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerProcessesInfo {
    pub(crate) total_count: usize,
    pub(crate) processes: Vec<ContainerProcessInfo>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerPortInfo {
    pub(crate) protocol: String,
    pub(crate) address: String,
    pub(crate) port: u16,
    pub(crate) process_name: Option<String>,
    pub(crate) pid: Option<u32>,
    pub(crate) source: String,
}

impl ContainerInfo {
    pub(crate) fn collect() -> Self {
        let hostname = hostname();
        let container_id = detect_container_id();
        let image = std::env::var("CONTAINER_IMAGE").ok();
        let mounts = detect_mounts();

        Self {
            container_id,
            image,
            hostname,
            mounts,
        }
    }
}

impl ContainerRuntimeInfo {
    pub(crate) fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let disks = sysinfo::Disks::new_with_refreshed_list();
        let (disk_total_bytes, disk_used_bytes) = disks.iter().fold((0u64, 0u64), |acc, disk| {
            (
                acc.0 + disk.total_space(),
                acc.1 + (disk.total_space() - disk.available_space()),
            )
        });
        let load_average = System::load_average();

        Self {
            hostname: hostname(),
            uptime_seconds: System::uptime(),
            process_count: sys.processes().len(),
            load_average_1m: load_average.one,
            load_average_5m: load_average.five,
            load_average_15m: load_average.fifteen,
            memory_total_bytes: sys.total_memory(),
            memory_used_bytes: sys.used_memory(),
            disk_total_bytes,
            disk_used_bytes,
            environment: EnvironmentKind::detect(),
        }
    }
}

impl ContainerProcessesInfo {
    pub(crate) fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let mut processes: Vec<ContainerProcessInfo> = sys
            .processes()
            .values()
            .map(|process| ContainerProcessInfo {
                pid: process.pid().as_u32(),
                name: process.name().to_string(),
                status: format!("{:?}", process.status()).to_lowercase(),
                cpu_usage_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
                virtual_memory_bytes: process.virtual_memory(),
                command: if process.cmd().is_empty() {
                    None
                } else {
                    Some(process.cmd().join(" "))
                },
                executable_path: process.exe().map(|path| path.display().to_string()),
            })
            .collect();

        processes.sort_by(|left, right| {
            right
                .memory_bytes
                .cmp(&left.memory_bytes)
                .then_with(|| right.cpu_usage_percent.total_cmp(&left.cpu_usage_percent))
                .then_with(|| left.pid.cmp(&right.pid))
        });

        let total_count = processes.len();
        processes.truncate(PROCESS_LIMIT);

        Self {
            total_count,
            processes,
        }
    }
}

pub(crate) fn collect_ports() -> Vec<ContainerPortInfo> {
    let mut ports = collect_ports_via_ss();
    if ports.is_empty() {
        ports = collect_ports_via_procfs();
    }
    ports.sort();
    ports.dedup();
    ports
}

fn hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| {
        sysinfo::System::host_name().unwrap_or_else(|| String::from("unknown"))
    })
}

fn detect_container_id() -> Option<String> {
    fs::read_to_string("/proc/1/cgroup")
        .ok()
        .and_then(|cgroup| {
            cgroup.lines().find_map(|line| {
                let id = line.rsplit('/').next()?;
                if id.len() >= 12 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                    Some(id[..12].to_string())
                } else {
                    None
                }
            })
        })
}

fn detect_mounts() -> Vec<String> {
    fs::read_to_string("/proc/mounts")
        .ok()
        .map(|content| {
            content
                .lines()
                .filter(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 3 {
                        return false;
                    }
                    !matches!(
                        parts[2],
                        "proc"
                            | "sysfs"
                            | "devpts"
                            | "tmpfs"
                            | "cgroup"
                            | "cgroup2"
                            | "mqueue"
                            | "devtmpfs"
                            | "securityfs"
                            | "debugfs"
                            | "pstore"
                            | "fusectl"
                            | "hugetlbfs"
                            | "bpf"
                    )
                })
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    parts.get(1).map(|path| (*path).to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn collect_ports_via_ss() -> Vec<ContainerPortInfo> {
    let mut ports = Vec::new();
    ports.extend(run_ss("tcp", &["-H", "-ltnp"]));
    ports.extend(run_ss("udp", &["-H", "-lunp"]));
    ports
}

fn run_ss(protocol: &str, args: &[&str]) -> Vec<ContainerPortInfo> {
    let Ok(output) = Command::new("ss").args(args).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| parse_ss_line(protocol, line))
        .collect()
}

fn parse_ss_line(protocol: &str, line: &str) -> Option<ContainerPortInfo> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    if columns.len() < 5 {
        return None;
    }

    let (local_index, process_index) = if matches!(
        columns.first().copied(),
        Some("tcp" | "tcp6" | "udp" | "udp6")
    ) {
        (4, 6)
    } else {
        (3, 5)
    };
    let local = *columns.get(local_index)?;
    let (address, port) = parse_socket_address(local)?;
    let process_column = if columns.len() > process_index {
        Some(columns[process_index..].join(" "))
    } else {
        None
    };
    let (process_name, pid) = process_column
        .as_deref()
        .map(parse_process_metadata)
        .unwrap_or((None, None));

    Some(ContainerPortInfo {
        protocol: protocol.to_string(),
        address,
        port,
        process_name,
        pid,
        source: String::from("ss"),
    })
}

fn parse_process_metadata(raw: &str) -> (Option<String>, Option<u32>) {
    let name = raw
        .split('"')
        .nth(1)
        .map(str::to_string)
        .filter(|value| !value.is_empty());
    let pid = raw
        .split("pid=")
        .nth(1)
        .and_then(|value| value.split([',', ')']).next())
        .and_then(|value| value.parse::<u32>().ok());

    (name, pid)
}

fn collect_ports_via_procfs() -> Vec<ContainerPortInfo> {
    let mut ports = Vec::new();
    ports.extend(parse_proc_net("/proc/net/tcp", "tcp"));
    ports.extend(parse_proc_net("/proc/net/tcp6", "tcp6"));
    ports.extend(parse_proc_net("/proc/net/udp", "udp"));
    ports.extend(parse_proc_net("/proc/net/udp6", "udp6"));
    ports
}

fn parse_proc_net(path: &str, protocol: &str) -> Vec<ContainerPortInfo> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            let local = columns.get(1)?;
            let state = columns.get(3)?;
            if protocol.starts_with("tcp") && *state != "0A" {
                return None;
            }
            let (address, port) = parse_proc_socket_address(local, protocol.contains('6'))?;
            Some(ContainerPortInfo {
                protocol: protocol.to_string(),
                address,
                port,
                process_name: None,
                pid: None,
                source: String::from("procfs"),
            })
        })
        .collect()
}

fn parse_socket_address(value: &str) -> Option<(String, u16)> {
    if value.is_empty() {
        return None;
    }

    let (address, port) = if let Some(stripped) = value.strip_prefix('[') {
        let (address, port) = stripped.split_once("]:")?;
        (address, port)
    } else {
        value.rsplit_once(':')?
    };
    let port = port.parse::<u16>().ok()?;

    Some((address.to_string(), port))
}

fn parse_proc_socket_address(value: &str, is_ipv6: bool) -> Option<(String, u16)> {
    let (address_hex, port_hex) = value.split_once(':')?;
    let port = u16::from_str_radix(port_hex, 16).ok()?;
    let address = if is_ipv6 {
        decode_ipv6_address(address_hex)?
    } else {
        decode_ipv4_address(address_hex)?
    };

    Some((address, port))
}

fn decode_ipv4_address(value: &str) -> Option<String> {
    if value.len() != 8 {
        return None;
    }

    let bytes = (0..4)
        .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;

    Some(
        bytes
            .into_iter()
            .rev()
            .map(|byte| byte.to_string())
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn decode_ipv6_address(value: &str) -> Option<String> {
    if value.len() != 32 {
        return None;
    }

    let address = (0..16)
        .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    let address = std::net::Ipv6Addr::from(<[u8; 16]>::try_from(address).ok()?);

    Some(address.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_proc_socket_address, parse_process_metadata, parse_socket_address, parse_ss_line,
    };

    #[test]
    fn parses_ipv4_socket_address() {
        assert_eq!(
            parse_socket_address("127.0.0.1:3000"),
            Some((String::from("127.0.0.1"), 3000))
        );
    }

    #[test]
    fn parses_ipv6_socket_address() {
        assert_eq!(
            parse_socket_address("[::]:8080"),
            Some((String::from("::"), 8080))
        );
    }

    #[test]
    fn parses_process_metadata_from_ss_output() {
        assert_eq!(
            parse_process_metadata("users:((\"node\",pid=42,fd=19))"),
            (Some(String::from("node")), Some(42))
        );
    }

    #[test]
    fn parses_ss_line() {
        let entry = parse_ss_line(
            "tcp",
            "tcp LISTEN 0 4096 127.0.0.1:3000 0.0.0.0:* users:((\"node\",pid=42,fd=19))",
        )
        .expect("expected parsed port entry");

        assert_eq!(entry.protocol, "tcp");
        assert_eq!(entry.address, "127.0.0.1");
        assert_eq!(entry.port, 3000);
        assert_eq!(entry.process_name.as_deref(), Some("node"));
        assert_eq!(entry.pid, Some(42));
        assert_eq!(entry.source, "ss");
    }

    #[test]
    fn parses_real_ss_line_without_protocol_column() {
        let entry = parse_ss_line(
            "tcp",
            "LISTEN 0 511 127.0.0.1:18789 0.0.0.0:* users:((\"openclaw-gatewa\",pid=820,fd=24))",
        )
        .expect("expected parsed port entry");

        assert_eq!(entry.protocol, "tcp");
        assert_eq!(entry.address, "127.0.0.1");
        assert_eq!(entry.port, 18789);
        assert_eq!(entry.process_name.as_deref(), Some("openclaw-gatewa"));
        assert_eq!(entry.pid, Some(820));
        assert_eq!(entry.source, "ss");
    }

    #[test]
    fn parses_proc_ipv4_socket_address() {
        assert_eq!(
            parse_proc_socket_address("0100007F:0BB8", false),
            Some((String::from("127.0.0.1"), 3000))
        );
    }

    #[test]
    fn parses_proc_ipv6_socket_address() {
        assert_eq!(
            parse_proc_socket_address("00000000000000000000000000000001:1F90", true),
            Some((String::from("::1"), 8080))
        );
    }
}
