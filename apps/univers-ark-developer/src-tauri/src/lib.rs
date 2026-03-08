mod app;
mod cleanup;
mod commands;
mod config;
mod constants;
mod files;
mod github;
mod models;
mod proxy;
mod runtime;
mod settings;
mod shell;
mod terminal;
mod tunnel;

pub fn run() {
    app::run();
}
