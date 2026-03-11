mod activity;
mod app;
mod cleanup;
mod commands;
mod config;
mod connectivity;
mod constants;
mod dashboard;
mod files;
mod github;
mod models;
mod proxy;
mod runtime;
mod scheduler;
mod secret_management;
mod service_registry;
mod settings;
mod shell;
mod terminal;
mod tunnel;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
