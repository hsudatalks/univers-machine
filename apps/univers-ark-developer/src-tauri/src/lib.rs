mod activity;
mod app;
mod cleanup;
mod commands;
mod connectivity;
mod constants;
mod dashboard;
mod files;
mod github;
mod infra;
mod machine;
mod models;
mod proxy;
mod scheduler;
mod secrets;
mod services;
mod settings;
mod shell;
mod terminal;
mod tunnel;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
