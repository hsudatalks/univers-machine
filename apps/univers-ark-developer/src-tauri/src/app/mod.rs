mod lifecycle;
mod menu;

use self::lifecycle::{handle_run_event, manage_app_state, setup_app};
use crate::commands;

#[cfg(desktop)]
use self::menu::{build_app_menu, handle_menu_event};

pub(crate) fn run() {
    let builder =
        manage_app_state(tauri::Builder::default()).plugin(tauri_plugin_clipboard_manager::init());
    #[cfg(desktop)]
    let builder = builder
        // NOTE: Keyboard shortcuts for container navigation (Ctrl+Alt+Left/Right/Up
        // on Windows, Cmd+Alt+Left/Right/Up on macOS) are handled by the menu
        // accelerators in build_app_menu, not by global shortcuts.
        .menu(build_app_menu)
        .on_menu_event(|app_handle, event| {
            handle_menu_event(app_handle, event.id().as_ref());
        });

    #[cfg(mobile)]
    let builder = builder;

    builder
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            commands::machine::bootstrap::load_bootstrap,
            commands::machine::bootstrap::refresh_bootstrap,
            commands::machine::bootstrap::load_machine_inventory,
            commands::machine::bootstrap::refresh_machine_inventory,
            commands::machine::bootstrap::scan_machine_inventory,
            commands::machine::import::scan_ssh_config_machine_candidates,
            commands::machine::import::scan_tailscale_machine_candidates,
            commands::runtime::browser::capture_browser_screenshot,
            commands::runtime::terminal::attach_terminal,
            commands::runtime::terminal::restart_terminal,
            commands::runtime::tunnel::ensure_tunnel,
            commands::runtime::tunnel::sync_tunnel_registrations,
            commands::runtime::tunnel::restart_tunnel,
            commands::runtime::tunnel::restart_all_tunnels,
            commands::runtime::terminal::list_remote_directory,
            commands::runtime::terminal::read_remote_file_preview,
            commands::runtime::dashboard::load_container_dashboard,
            commands::runtime::dashboard::start_dashboard_monitor,
            commands::runtime::dashboard::stop_dashboard_monitor,
            commands::runtime::dashboard::refresh_container_dashboard,
            commands::github::load_github_project_state,
            commands::github::load_github_pull_request_detail,
            commands::github::merge_github_pull_request,
            commands::github::open_external_link,
            commands::runtime::terminal::write_terminal,
            commands::runtime::terminal::resize_terminal,
            commands::machine::actions::restart_container,
            commands::runtime::misc::restart_tmux,
            commands::runtime::misc::execute_command_service,
            commands::runtime::misc::clipboard_write,
            commands::runtime::misc::clipboard_read,
            commands::machine::config::load_targets_config,
            commands::machine::config::update_targets_config,
            commands::machine::config::load_machine_config_state,
            commands::machine::config::upsert_machine_config,
            commands::machine::config::delete_machine_config,
            commands::settings::load_app_settings,
            commands::diagnostics::load_app_diagnostics,
            commands::secrets::load_secret_inventory,
            commands::secrets::upsert_secret_provider,
            commands::secrets::delete_secret_provider,
            commands::secrets::upsert_secret_credential,
            commands::secrets::delete_secret_credential,
            commands::secrets::upsert_secret_assignment,
            commands::secrets::delete_secret_assignment,
            commands::settings::save_app_settings,
            commands::runtime::misc::update_runtime_activity,
            commands::runtime::vnc::start_vnc_session,
            commands::runtime::vnc::stop_vnc_session
        ])
        .build(tauri::generate_context!())
        .expect("error while building univers-ark-developer")
        .run(handle_run_event);
}
