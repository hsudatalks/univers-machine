use super::BrowserScreenshotRectInput;
use crate::{
    models::BrowserScreenshotCapture,
    runtime::browser::{
        capture_browser_screenshot as run_browser_screenshot_capture, BrowserScreenshotRect,
    },
};
use tauri::async_runtime;

#[tauri::command]
pub(crate) async fn capture_browser_screenshot(
    target_id: String,
    service_id: String,
    rect: BrowserScreenshotRectInput,
) -> Result<BrowserScreenshotCapture, String> {
    async_runtime::spawn_blocking(move || {
        run_browser_screenshot_capture(
            &target_id,
            &service_id,
            &BrowserScreenshotRect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            },
        )
    })
    .await
    .map_err(|error| format!("Failed to join browser screenshot task: {error}"))?
}
