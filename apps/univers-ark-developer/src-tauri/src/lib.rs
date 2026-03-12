mod app;
mod commands;
mod constants;
mod github;
mod infra;
mod machine;
mod models;
mod runtime;
mod secrets;
mod services;
mod settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
