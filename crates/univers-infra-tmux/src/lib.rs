use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::{Command, Output};

#[derive(Debug, Clone, Default)]
pub struct TmuxGateway;

impl TmuxGateway {
    pub fn session_exists(&self, server: Option<&str>, session: &str) -> bool {
        self.tmux_command(server)
            .args(["has-session", "-t", session])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    pub fn window_exists(&self, server: Option<&str>, session: &str, window: &str) -> bool {
        let Ok(output) = self
            .tmux_command(server)
            .args(["list-windows", "-t", session, "-F", "#{window_name}"])
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.trim() == window)
    }

    pub fn session_attached(&self, server: Option<&str>, session: &str) -> bool {
        let Ok(output) = self
            .tmux_command(server)
            .args([
                "list-sessions",
                "-F",
                "#{session_name}\t#{?session_attached,1,0}",
            ])
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let (name, attached) = line.split_once('\t')?;
                Some((name.trim(), attached.trim() == "1"))
            })
            .find_map(|(name, attached)| (name == session).then_some(attached))
            .unwrap_or(false)
    }

    pub fn session_active_command(&self, server: Option<&str>, session: &str) -> Option<String> {
        let Ok(output) = self
            .tmux_command(server)
            .args([
                "list-panes",
                "-t",
                session,
                "-F",
                "#{?pane_active,1,0}\t#{pane_current_command}",
            ])
            .output()
        else {
            return None;
        };
        if !output.status.success() {
            return None;
        }

        let mut fallback = None;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some((active, command)) = line.split_once('\t') else {
                continue;
            };
            let command = command.trim();
            if command.is_empty() {
                continue;
            }
            if active.trim() == "1" {
                return Some(command.to_string());
            }
            if fallback.is_none() {
                fallback = Some(command.to_string());
            }
        }

        fallback
    }

    pub fn new_session(
        &self,
        server: Option<&str>,
        session: &str,
        window_name: &str,
        working_directory: &Path,
        window_command: Option<&str>,
    ) -> Result<Output> {
        let mut command = self.tmux_command(server);
        command.args([
            "new-session",
            "-d",
            "-s",
            session,
            "-n",
            window_name,
            "-c",
            &working_directory.display().to_string(),
        ]);
        if let Some(window_command) = window_command {
            command.args(["sh", "-lc", window_command]);
        }
        Ok(command.output()?)
    }

    pub fn new_window(
        &self,
        server: Option<&str>,
        session: &str,
        window_name: &str,
        working_directory: &Path,
        window_command: Option<&str>,
    ) -> Result<Output> {
        let mut command = self.tmux_command(server);
        command.args([
            "new-window",
            "-t",
            session,
            "-n",
            window_name,
            "-c",
            &working_directory.display().to_string(),
        ]);
        if let Some(window_command) = window_command {
            command.args(["sh", "-lc", window_command]);
        }
        Ok(command.output()?)
    }

    pub fn kill_session(&self, server: Option<&str>, session: &str) -> Result<()> {
        let output = self
            .tmux_command(server)
            .args(["kill-session", "-t", session])
            .output()
            .with_context(|| format!("Failed to stop workspace '{session}'"))?;
        if output.status.success() {
            return Ok(());
        }

        Err(anyhow!(stderr_or_default(
            &output,
            &format!("tmux kill-session failed for '{session}'"),
        )))
    }

    pub fn kill_window(&self, server: Option<&str>, session: &str, window: &str) -> Result<()> {
        let target = format!("{session}:{window}");
        let output = self
            .tmux_command(server)
            .args(["kill-window", "-t", &target])
            .output()
            .with_context(|| {
                format!("Failed to stop window '{window}' in workspace '{session}'")
            })?;
        if output.status.success() {
            return Ok(());
        }

        Err(anyhow!(stderr_or_default(
            &output,
            &format!("tmux kill-window failed for '{target}'"),
        )))
    }

    pub fn capture_logs(
        &self,
        server: Option<&str>,
        session: &str,
        window: Option<&str>,
    ) -> Result<String> {
        if !self.session_exists(server, session) {
            let server_label = server.unwrap_or("default");
            return Err(anyhow!(
                "tmux workspace '{session}' is not running on server '{server_label}'"
            ));
        }

        let target = match window {
            Some(window) => format!("{session}:{window}"),
            None => format!("{session}:0"),
        };
        let output = self
            .tmux_command(server)
            .args(["capture-pane", "-t", &target, "-p", "-S", "-200"])
            .output()
            .with_context(|| format!("Failed to capture logs for tmux target '{target}'"))?;

        if !output.status.success() {
            return Err(anyhow!(stderr_or_default(
                &output,
                &format!("tmux capture-pane failed for '{target}'"),
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn tmux_command(&self, server: Option<&str>) -> Command {
        let mut command = Command::new("tmux");
        if let Some(server) = server.filter(|server| !server.is_empty() && *server != "default") {
            command.args(["-L", server]);
        }
        command
    }
}

fn stderr_or_default(output: &Output, default_message: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        default_message.to_string()
    } else {
        stderr
    }
}
