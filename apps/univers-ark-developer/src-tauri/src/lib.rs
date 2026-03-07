mod app;
mod cleanup;
mod commands;
mod config;
mod constants;
mod files;
mod models;
mod proxy;
mod runtime;
mod terminal;
mod tunnel;

pub fn run() {
    app::run();
}
