use anyhow::{Context, Result};
use tokio::process::Command;

/// Run a command and return its stdout as a trimmed string.
pub async fn run_cmd(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to run: {program} {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "{program} {} failed (exit {}): {stderr}",
            args.join(" "),
            output.status
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if a command exists on PATH.
pub async fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Extract version from a command's output (e.g., "node v20.11.0" → "v20.11.0").
pub async fn get_version(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Find first version-like token
    for token in stdout.split_whitespace() {
        if token.starts_with('v') || token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some(token.trim().to_string());
        }
    }
    Some(stdout.trim().to_string())
}

/// Detect the system package manager.
pub async fn detect_pkg_manager() -> Option<&'static str> {
    if command_exists("apt-get").await {
        Some("apt")
    } else if command_exists("dnf").await {
        Some("dnf")
    } else if command_exists("yum").await {
        Some("yum")
    } else if command_exists("pacman").await {
        Some("pacman")
    } else if command_exists("apk").await {
        Some("apk")
    } else if command_exists("brew").await {
        Some("brew")
    } else {
        None
    }
}

/// Run a shell command string via sh -c.
pub async fn run_shell(cmd: &str) -> Result<String> {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .output()
        .await
        .with_context(|| format!("Failed to run shell: {cmd}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Shell command failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
