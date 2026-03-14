use crate::{
    infra::ssh::execute_chain_blocking,
    machine::{resolve_raw_target, resolve_target_ssh_chain, run_target_shell_command},
    models::DeveloperTarget,
    services::health::dashboard_probe_command,
};
use univers_infra_ssh::ClientOptions as RusshClientOptions;

const DEFAULT_PROJECT_PATH: &str = "~/repos";

fn target_project_path(target: &DeveloperTarget) -> &str {
    let project_path = target.workspace.project_path.trim();
    if !project_path.is_empty() {
        return project_path;
    }

    let files_root = target.workspace.files_root.trim();
    if !files_root.is_empty() {
        return files_root;
    }

    DEFAULT_PROJECT_PATH
}

fn dashboard_command(target: &DeveloperTarget) -> Result<String, String> {
    dashboard_probe_command(target, target_project_path(target))
}

pub(super) fn load_container_dashboard_stdout(target_id: &str) -> Result<Vec<u8>, String> {
    let target = resolve_raw_target(target_id)?;
    let command = dashboard_command(&target)?;

    if let Ok(stdout) = load_container_dashboard_via_russh(target_id, &command) {
        return Ok(stdout);
    }

    let output = run_target_shell_command(target_id, &command)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("Dashboard command failed for {target_id}")
        });
    }

    Ok(output.stdout)
}

fn load_container_dashboard_via_russh(target_id: &str, command: &str) -> Result<Vec<u8>, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
    let output = execute_chain_blocking(&chain, command, &RusshClientOptions::default())
        .map_err(|error| format!("russh dashboard exec failed for {target_id}: {error}"))?;

    if output.exit_status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("russh dashboard command failed for {target_id}")
        });
    }

    Ok(output.stdout)
}
