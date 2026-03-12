use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

const REPO_NAME: &str = "hvac-workbench";

pub(super) fn hvac_workbench_repo_path() -> Option<PathBuf> {
    let home = if cfg!(target_os = "windows") {
        env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        env::var_os("HOME").map(PathBuf::from)
    }?;
    let candidate = home.join("repos").join(REPO_NAME);
    candidate.exists().then_some(candidate)
}

pub(super) fn local_branch(repo_path: &Path) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("branch")
        .arg("--show-current")
        .output()
        .map_err(|error| format!("Failed to inspect {} branch: {}", REPO_NAME, error))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("Failed to inspect {} branch", REPO_NAME)
        } else {
            stderr
        });
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!branch.is_empty()).then_some(branch))
}

pub(super) fn local_status_summary(repo_path: &Path) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("status")
        .arg("--short")
        .arg("--branch")
        .output()
        .map_err(|error| format!("Failed to inspect {} status: {}", REPO_NAME, error))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("Failed to inspect {} status", REPO_NAME)
        } else {
            stderr
        });
    }

    let summary = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from);

    Ok(summary)
}
