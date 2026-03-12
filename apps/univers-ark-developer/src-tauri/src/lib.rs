mod app;
mod cleanup;
mod commands;
mod constants;
mod files;
mod github;
mod infra;
mod machine;
mod models;
mod proxy;
mod runtime;
mod secrets;
mod services;
mod settings;
mod shell;
mod terminal;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
