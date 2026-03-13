use crate::{
    machine::resolve_raw_target, models::BrowserScreenshotCapture,
    runtime::files::write_remote_file,
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone)]
pub(crate) struct BrowserScreenshotRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) fn capture_browser_screenshot(
    target_id: &str,
    service_id: &str,
    rect: &BrowserScreenshotRect,
) -> Result<BrowserScreenshotCapture, String> {
    if rect.width == 0 || rect.height == 0 {
        return Err(String::from("Browser screenshot area is empty."));
    }

    let target = resolve_raw_target(target_id)?;
    let remote_path = build_remote_screenshot_path(&target.label, service_id);
    let temp_path = capture_local_screenshot(rect)?;
    let screenshot_bytes = fs::read(&temp_path)
        .map_err(|error| format!("Failed to read captured screenshot: {error}"))?;
    let _ = fs::remove_file(&temp_path);

    write_remote_file(target_id, &remote_path, &screenshot_bytes)?;

    Ok(BrowserScreenshotCapture {
        target_id: target_id.to_string(),
        path: remote_path,
    })
}

fn build_remote_screenshot_path(target_label: &str, service_id: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let target_slug = slugify(target_label);
    let service_slug = slugify(service_id);

    format!(
        "~/Pictures/ark-console/{}-{}-{}.png",
        if target_slug.is_empty() {
            "workbench"
        } else {
            target_slug.as_str()
        },
        if service_slug.is_empty() {
            "browser"
        } else {
            service_slug.as_str()
        },
        timestamp
    )
}

fn slugify(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            output.push('-');
            previous_was_dash = true;
        }
    }

    output.trim_matches('-').to_string()
}

#[cfg(target_os = "macos")]
fn capture_local_screenshot(rect: &BrowserScreenshotRect) -> Result<PathBuf, String> {
    let temp_path = std::env::temp_dir().join(format!(
        "ark-console-browser-{}-{}.png",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    ));

    let region = format!("{},{},{},{}", rect.x, rect.y, rect.width, rect.height);
    let output = Command::new("screencapture")
        .arg("-x")
        .arg("-R")
        .arg(region)
        .arg(&temp_path)
        .output()
        .map_err(|error| format!("Failed to start screencapture: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "screencapture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(temp_path)
}

#[cfg(not(target_os = "macos"))]
fn capture_local_screenshot(_rect: &BrowserScreenshotRect) -> Result<PathBuf, String> {
    Err(String::from(
        "Browser screenshots are currently implemented on macOS only.",
    ))
}
