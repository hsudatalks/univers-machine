use crate::{
    machine::{execute_target_command_via_russh, resolve_raw_target},
    services::{catalog::command_service, registry::emit_command_service_status},
};
use tauri::{AppHandle, Runtime};

pub(crate) fn execute_command_service_action<R: Runtime>(
    app: Option<&AppHandle<R>>,
    target_id: &str,
    service_id: &str,
    action: &str,
) -> Result<(), String> {
    let target = resolve_raw_target(target_id)?;
    let service = command_service(&target, service_id).ok_or_else(|| {
        format!(
            "Unknown command service {service_id} for target {target_id}"
        )
    })?;

    let command = match action {
        "restart" => service
            .command
            .as_ref()
            .map(|command| command.restart.trim())
            .filter(|command| !command.is_empty())
            .ok_or_else(|| {
                format!(
                    "Command service {service_id} does not define a restart action"
                )
            })?,
        other => {
            return Err(format!(
                "Unsupported command service action {other} for {service_id}"
            ));
        }
    };

    if let Some(app) = app {
        emit_command_service_status(
            app,
            target_id,
            service_id,
            "running",
            format!("Executing {action} action."),
        );
    }

    let output = execute_target_command_via_russh(target_id, command)?;
    let (exit_status, stdout, stderr) = (output.exit_status, output.stdout, output.stderr);

    if exit_status == 0 {
        if let Some(app) = app {
            emit_command_service_status(
                app,
                target_id,
                service_id,
                "ready",
                format!("{action} action finished successfully."),
            );
        }
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&stdout).trim().to_string();

    let error = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("Failed to execute {action} action for {service_id}")
    };

    if let Some(app) = app {
        emit_command_service_status(app, target_id, service_id, "error", error.clone());
    }

    Err(error)
}
