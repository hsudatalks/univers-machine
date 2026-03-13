use crate::models::{DeveloperTarget, EndpointProbeType};
use serde::Serialize;
use url::Url;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeclaredDashboardService {
    id: String,
    label: String,
    probe_type: EndpointProbeType,
    host: String,
    port: u16,
    path: String,
    url: String,
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn declared_dashboard_services(target: &DeveloperTarget) -> Vec<DeclaredDashboardService> {
    target
        .services
        .iter()
        .filter_map(|service| {
            if let Some(endpoint) = service.endpoint.as_ref() {
                return Some(DeclaredDashboardService {
                    id: service.id.clone(),
                    label: service.label.clone(),
                    probe_type: endpoint.probe_type,
                    host: if endpoint.host.trim().is_empty() {
                        String::from("127.0.0.1")
                    } else {
                        endpoint.host.clone()
                    },
                    port: endpoint.port,
                    path: endpoint.path.clone(),
                    url: endpoint.url.clone(),
                });
            }

            let web = service.web.as_ref()?;
            let parsed = Url::parse(&web.remote_url).ok()?;
            let host = parsed.host_str().unwrap_or("127.0.0.1").to_string();
            let port = parsed.port_or_known_default()?;
            let mut path = parsed.path().to_string();

            if path == "/" {
                path.clear();
            }

            Some(DeclaredDashboardService {
                id: service.id.clone(),
                label: service.label.clone(),
                probe_type: EndpointProbeType::Http,
                host,
                port,
                path,
                url: web.remote_url.clone(),
            })
        })
        .collect()
}

pub(crate) fn dashboard_probe_command(
    target: &DeveloperTarget,
    project_path: &str,
) -> Result<String, String> {
    let declared_services = serde_json::to_string(&declared_dashboard_services(target))
        .map_err(|error| format!("Failed to serialize declared services: {error}"))?;

    Ok(format!(
        r##"UNIVERS_ARK_PROJECT_PATH={} UNIVERS_ARK_DECLARED_SERVICES={} python3 - <<'PY'
import json
import os
import socket
import shutil
import subprocess
import time
from urllib.parse import urlparse

project_path = os.path.abspath(os.path.expanduser(os.environ.get("UNIVERS_ARK_PROJECT_PATH") or "{default_project}"))
declared_services_json = os.environ.get("UNIVERS_ARK_DECLARED_SERVICES") or "[]"
repo_found = os.path.isdir(project_path)
env_path = os.path.join(project_path, ".env")

branch = None
is_dirty = False
changed_files = 0
head_summary = None
services = []
declared_service_defs = []

try:
    parsed_declared = json.loads(declared_services_json)
    if isinstance(parsed_declared, list):
        declared_service_defs = parsed_declared
except Exception:
    declared_service_defs = []

if repo_found and os.path.isdir(os.path.join(project_path, ".git")):
    def run_git(*args):
        return subprocess.run(
            ["git", *args],
            cwd=project_path,
            capture_output=True,
            text=True,
            check=False,
        )

    branch_result = run_git("rev-parse", "--abbrev-ref", "HEAD")
    if branch_result.returncode == 0:
        branch = branch_result.stdout.strip() or None

    status_result = run_git("status", "--porcelain")
    if status_result.returncode == 0:
        changed_lines = [line for line in status_result.stdout.splitlines() if line.strip()]
        changed_files = len(changed_lines)
        is_dirty = changed_files > 0

    head_result = run_git("log", "-1", "--pretty=%h %s")
    if head_result.returncode == 0:
        head_summary = head_result.stdout.strip() or None

env_values = {{}}
if os.path.isfile(env_path):
    with open(env_path, "r", encoding="utf-8") as handle:
        for raw_line in handle:
            line = raw_line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            env_values[key.strip()] = value.strip().strip('"').strip("'")

def tcp_reachable(host, port, timeout=0.5):
    try:
        with socket.create_connection((host, int(port)), timeout=timeout):
            return True
    except OSError:
        return False

def http_service_status(label, host, port, path="/health", url=None):
    target_url = url or f"http://{{host}}:{{port}}"
    healthy = tcp_reachable(host, port)
    status = "running" if healthy else "down"
    detail = f"{{host}}:{{port}}{{path if path else ''}}"
    if healthy:
        probe = subprocess.run(
            [
                "python3",
                "-c",
                "import sys, urllib.request; "
                "url=sys.argv[1]; "
                "resp=urllib.request.urlopen(url, timeout=1); "
                "print(resp.status)",
                f"{{target_url}}{{path}}",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if probe.returncode == 0 and probe.stdout.strip():
            detail = f"HTTP {{probe.stdout.strip()}} · {{host}}:{{port}}{{path if path else ''}}"
        elif path:
            detail = f"TCP ready · {{host}}:{{port}}"
    return {{
        "id": label.lower().replace(" ", "-"),
        "label": label,
        "status": status,
        "detail": detail,
        "url": target_url,
    }}

if declared_service_defs:
    for definition in declared_service_defs:
        host = (definition.get("host") or "127.0.0.1").strip() or "127.0.0.1"
        port = int(definition.get("port") or 0)
        if port <= 0:
            continue
        label = (definition.get("label") or definition.get("id") or f"service-{{port}}").strip()
        service_id = (definition.get("id") or label.lower().replace(" ", "-")).strip()
        probe_type = (definition.get("probeType") or "http").strip().lower()
        path = definition.get("path") or ""
        url = definition.get("url") or (f"http://{{host}}:{{port}}" if probe_type == "http" else "")

        if probe_type == "tcp":
            ready = tcp_reachable(host, port)
            services.append(
                {{
                    "id": service_id,
                    "label": label,
                    "status": "running" if ready else "down",
                    "detail": f"tcp://{{host}}:{{port}}",
                    "url": url or None,
                }}
            )
        else:
            services.append(http_service_status(label, host, port, path=path, url=url or None))
            services[-1]["id"] = service_id
else:
    database_url = env_values.get("DATABASE_URL", "memory").strip()
    if database_url.lower() == "memory":
        services.append(
            {{
                "id": "surrealdb",
                "label": "SurrealDB",
                "status": "embedded",
                "detail": "Embedded memory database",
                "url": None,
            }}
        )
    else:
        parsed = urlparse(database_url)
        db_host = parsed.hostname or "127.0.0.1"
        db_port = parsed.port or 8000
        db_ready = tcp_reachable(db_host, db_port)
        services.append(
            {{
                "id": "surrealdb",
                "label": "SurrealDB",
                "status": "running" if db_ready else "down",
                "detail": f"{{parsed.scheme or 'tcp'}}://{{db_host}}:{{db_port}}",
                "url": f"http://{{db_host}}:{{db_port}}" if db_ready else None,
            }}
        )

    server_port = int(env_values.get("SERVER_PORT", "3003") or "3003")
    agents_port = int(
        env_values.get("AGENTS_PORT")
        or env_values.get("COPILOT_PORT")
        or str(server_port + 1)
    )
    web_port = 3432

    services.append(http_service_status("Workbench API", "127.0.0.1", server_port))
    services.append(http_service_status("Agents API", "127.0.0.1", agents_port))
    services.append(http_service_status("Web UI", "127.0.0.1", web_port, path=""))

def iso_timestamp(epoch_seconds):
    if not epoch_seconds:
        return None
    return time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(epoch_seconds))

tmux_info = {{
    "installed": False,
    "serverRunning": False,
    "sessionCount": 0,
    "attachedCount": 0,
    "activeSession": None,
    "activeCommand": None,
    "sessions": [],
}}

if shutil.which("tmux"):
    tmux_info["installed"] = True
    def collect_tmux_sessions(server_label, extra_args):
        tmux_sessions = subprocess.run(
            ["tmux", *extra_args, "list-sessions", "-F", "#{{session_name}}\t#{{session_windows}}\t#{{?session_attached,1,0}}"],
            capture_output=True,
            text=True,
            check=False,
        )
        if tmux_sessions.returncode != 0:
            return []

        pane_by_session = {{}}
        tmux_panes = subprocess.run(
            [
                "tmux",
                *extra_args,
                "list-panes",
                "-a",
                "-F",
                "#{{session_name}}\t#{{?pane_active,1,0}}\t#{{pane_current_command}}",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if tmux_panes.returncode == 0:
            for raw_line in tmux_panes.stdout.splitlines():
                parts = raw_line.split("\t")
                if len(parts) < 3:
                    continue
                session_name, is_active, current_command = parts[0], parts[1], parts[2].strip() or None
                existing = pane_by_session.get(session_name)
                if existing is None or is_active == "1":
                    pane_by_session[session_name] = current_command

        sessions = []
        for raw_line in tmux_sessions.stdout.splitlines():
            parts = raw_line.split("\t")
            if len(parts) < 3:
                continue
            name = parts[0]
            try:
                windows = int(parts[1])
            except ValueError:
                windows = 0
            attached = parts[2] == "1"
            sessions.append(
                {{
                    "server": server_label,
                    "name": name,
                    "windows": windows,
                    "attached": attached,
                    "activeCommand": pane_by_session.get(name),
                }}
            )
        return sessions

    default_sessions = collect_tmux_sessions("default", [])
    container_sessions = collect_tmux_sessions("container", ["-L", "container"])
    sessions = [*default_sessions, *container_sessions]

    if sessions:
        tmux_info["serverRunning"] = True
        tmux_info["sessions"] = sessions
        tmux_info["sessionCount"] = len(sessions)
        tmux_info["attachedCount"] = sum(1 for session in sessions if session["attached"])

        preferred_session = next(
            (session for session in sessions if session["attached"] and session["server"] == "default"),
            next((session for session in sessions if session["attached"]), sessions[0]),
        )
        tmux_info["activeSession"] = f"{{preferred_session['server']}} · {{preferred_session['name']}}"
        tmux_info["activeCommand"] = preferred_session.get("activeCommand")

agent_markers = [
    (".claude", "claude"),
    (".codex", "codex"),
    (".agents", "agent"),
    (".agent", "agent"),
]
latest_agent_marker = None
for relative_path, agent_name in agent_markers:
    marker_path = os.path.join(project_path, relative_path)
    if not os.path.exists(marker_path):
        continue
    try:
        modified_at = os.path.getmtime(marker_path)
    except OSError:
        continue
    if latest_agent_marker is None or modified_at > latest_agent_marker[2]:
        latest_agent_marker = (relative_path, agent_name, modified_at)

latest_report = None
if repo_found:
    skip_dirs = {{".git", "node_modules", "dist", "build", "target"}}
    for root, dirnames, filenames in os.walk(project_path):
        dirnames[:] = [name for name in dirnames if name not in skip_dirs]
        for filename in filenames:
            upper_name = filename.upper()
            if "REPORT" not in upper_name:
                continue
            if not filename.lower().endswith((".md", ".markdown", ".txt")):
                continue
            file_path = os.path.join(root, filename)
            try:
                modified_at = os.path.getmtime(file_path)
            except OSError:
                continue
            relative_path = os.path.relpath(file_path, project_path)
            if latest_report is None or modified_at > latest_report[1]:
                latest_report = (relative_path, modified_at)

active_agent = "unknown"
agent_source = "none"
last_activity = None

tmux_command = (tmux_info.get("activeCommand") or "").lower()
if "claude" in tmux_command and "codex" in tmux_command:
    active_agent = "mixed"
    agent_source = "tmux"
    last_activity = f"tmux · {{tmux_info.get('activeSession') or 'session'}} · {{tmux_info.get('activeCommand')}}"
elif "claude" in tmux_command:
    active_agent = "claude"
    agent_source = "tmux"
    last_activity = f"tmux · {{tmux_info.get('activeSession') or 'session'}} · {{tmux_info.get('activeCommand')}}"
elif "codex" in tmux_command:
    active_agent = "codex"
    agent_source = "tmux"
    last_activity = f"tmux · {{tmux_info.get('activeSession') or 'session'}} · {{tmux_info.get('activeCommand')}}"
elif latest_agent_marker is not None:
    active_agent = latest_agent_marker[1]
    agent_source = "workspace"
    last_activity = f"{{latest_agent_marker[0]}} · {{iso_timestamp(latest_agent_marker[2])}}"
elif latest_report is not None:
    active_agent = "agent"
    agent_source = "report"
    last_activity = latest_report[0]

agent_info = {{
    "activeAgent": active_agent,
    "source": agent_source,
    "lastActivity": last_activity,
    "latestReport": latest_report[0] if latest_report else None,
    "latestReportUpdatedAt": iso_timestamp(latest_report[1]) if latest_report else None,
}}

loadavg = os.getloadavg()
process_count = 0
if os.path.isdir("/proc"):
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            process_count += 1

if process_count == 0:
    ps = subprocess.run(
        ["sh", "-lc", "ps -A | wc -l"],
        capture_output=True,
        text=True,
        check=False,
    )
    if ps.returncode == 0:
        try:
            process_count = max(int(ps.stdout.strip()) - 1, 0)
        except ValueError:
            process_count = 0

mem_total = 0
mem_used = 0
if os.path.exists("/proc/meminfo"):
    meminfo = {{}}
    with open("/proc/meminfo", "r", encoding="utf-8") as handle:
        for line in handle:
            parts = line.split(":", 1)
            if len(parts) != 2:
                continue
            key = parts[0]
            value = parts[1].strip().split()[0]
            try:
                meminfo[key] = int(value) * 1024
            except ValueError:
                pass
    mem_total = meminfo.get("MemTotal", 0)
    mem_available = meminfo.get("MemAvailable", 0)
    mem_used = max(mem_total - mem_available, 0)
else:
    vm_stat = subprocess.run(["vm_stat"], capture_output=True, text=True, check=False)
    page_size = 4096
    if vm_stat.returncode == 0:
        free_pages = 0
        speculative_pages = 0
        active_pages = 0
        inactive_pages = 0
        wired_pages = 0
        for line in vm_stat.stdout.splitlines():
            if "page size of" in line:
                try:
                    page_size = int(line.split("page size of", 1)[1].split()[0])
                except Exception:
                    page_size = 4096
            if ":" not in line:
                continue
            key, raw = line.split(":", 1)
            try:
                value = int(raw.strip().rstrip(".").replace(".", ""))
            except ValueError:
                continue
            key = key.strip()
            if key == "Pages free":
                free_pages = value
            elif key == "Pages speculative":
                speculative_pages = value
            elif key == "Pages active":
                active_pages = value
            elif key == "Pages inactive":
                inactive_pages = value
            elif key == "Pages wired down":
                wired_pages = value
        sysctl = subprocess.run(
            ["sysctl", "-n", "hw.memsize"],
            capture_output=True,
            text=True,
            check=False,
        )
        if sysctl.returncode == 0:
            try:
                mem_total = int(sysctl.stdout.strip())
            except ValueError:
                mem_total = 0
        mem_used = (active_pages + inactive_pages + wired_pages) * page_size
        mem_used = max(mem_used - ((free_pages + speculative_pages) * page_size), 0)

disk_usage = shutil.disk_usage(project_path if repo_found else os.path.expanduser("~"))

print(json.dumps({{
    "project": {{
        "projectPath": project_path,
        "repoFound": repo_found,
        "branch": branch,
        "isDirty": is_dirty,
        "changedFiles": changed_files,
        "headSummary": head_summary,
    }},
    "runtime": {{
        "hostname": os.uname().nodename,
        "uptimeSeconds": int(float(open('/proc/uptime').read().split()[0])) if os.path.exists('/proc/uptime') else 0,
        "processCount": process_count,
        "loadAverage1m": round(loadavg[0], 2),
        "loadAverage5m": round(loadavg[1], 2),
        "loadAverage15m": round(loadavg[2], 2),
        "memoryTotalBytes": int(mem_total),
        "memoryUsedBytes": int(mem_used),
        "diskTotalBytes": int(disk_usage.total),
        "diskUsedBytes": int(disk_usage.used),
    }},
    "services": services,
    "agent": agent_info,
    "tmux": tmux_info,
}}, ensure_ascii=False))
PY"##,
        shell_single_quote(project_path),
        shell_single_quote(&declared_services),
        default_project = project_path,
    ))
}
