use crate::{
    cleanup::cleanup_stale_ssh_tunnels,
    commands,
    config::initialize_targets_file_path,
    dashboard::stop_all_dashboard_monitors,
    models::{DashboardState, TerminalState, TunnelState},
    tunnel::stop_all_tunnels,
};
use tauri::{
    menu::{
        AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
        WINDOW_SUBMENU_ID,
    },
    AppHandle, Emitter, Manager, Runtime,
};

const TOGGLE_SIDEBAR_MENU_ID: &str = "toggle_sidebar";
const TOGGLE_SIDEBAR_EVENT: &str = "toggle-sidebar-requested";

#[cfg(target_os = "macos")]
fn build_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let pkg_info = app_handle.package_info();
    let config = app_handle.config();
    let about_metadata = AboutMetadata {
        name: Some(pkg_info.name.clone()),
        version: Some(pkg_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config.bundle.publisher.clone().map(|publisher| vec![publisher]),
        ..Default::default()
    };

    let toggle_sidebar = MenuItem::with_id(
        app_handle,
        TOGGLE_SIDEBAR_MENU_ID,
        "Toggle Sidebar Menu",
        true,
        Some("Cmd+H"),
    )?;

    let app_menu = Submenu::with_items(
        app_handle,
        pkg_info.name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::services(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &toggle_sidebar,
            &PredefinedMenuItem::hide_others(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::quit(app_handle, None)?,
        ],
    )?;

    let file_menu = Submenu::with_items(
        app_handle,
        "File",
        true,
        &[&PredefinedMenuItem::close_window(app_handle, None)?],
    )?;

    let edit_menu = Submenu::with_items(
        app_handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app_handle, None)?,
            &PredefinedMenuItem::redo(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::cut(app_handle, None)?,
            &PredefinedMenuItem::copy(app_handle, None)?,
            &PredefinedMenuItem::paste(app_handle, None)?,
            &PredefinedMenuItem::select_all(app_handle, None)?,
        ],
    )?;

    let view_menu = Submenu::with_items(
        app_handle,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
    )?;

    let window_menu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;

    let help_menu =
        Submenu::with_id_and_items(app_handle, HELP_SUBMENU_ID, "Help", true, &[])?;

    Menu::with_items(
        app_handle,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ],
    )
}

#[cfg(not(target_os = "macos"))]
fn build_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let toggle_sidebar = MenuItem::with_id(
        app_handle,
        TOGGLE_SIDEBAR_MENU_ID,
        "Toggle Sidebar",
        true,
        Some("Ctrl+H"),
    )?;

    let file_menu = Submenu::with_items(
        app_handle,
        "File",
        true,
        &[
            &toggle_sidebar,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::quit(app_handle, None)?,
        ],
    )?;

    let view_menu = Submenu::with_items(
        app_handle,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
    )?;

    Menu::with_items(app_handle, &[&file_menu, &view_menu])
}

pub(crate) fn run() {
    tauri::Builder::default()
        .manage(TerminalState::default())
        .manage(TunnelState::default())
        .manage(DashboardState::default())
        .menu(build_app_menu)
        .on_menu_event(|app_handle, event| {
            if event.id().as_ref() == TOGGLE_SIDEBAR_MENU_ID {
                let _ = app_handle.emit(TOGGLE_SIDEBAR_EVENT, ());
            }
        })
        .setup(|app| {
            initialize_targets_file_path(app.handle())?;

            std::thread::spawn(|| match cleanup_stale_ssh_tunnels() {
                Ok(cleaned) if cleaned > 0 => {
                    eprintln!(
                        "Reaped {} stale managed SSH tunnel process(es) before startup.",
                        cleaned
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Failed to reap stale managed SSH tunnels: {}", error);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_bootstrap,
            commands::refresh_bootstrap,
            commands::load_server_inventory,
            commands::refresh_server_inventory,
            commands::attach_terminal,
            commands::restart_terminal,
            commands::ensure_tunnel,
            commands::restart_tunnel,
            commands::list_remote_directory,
            commands::read_remote_file_preview,
            commands::load_container_dashboard,
            commands::start_dashboard_monitor,
            commands::stop_dashboard_monitor,
            commands::refresh_container_dashboard,
            commands::load_github_project_state,
            commands::load_github_pull_request_detail,
            commands::merge_github_pull_request,
            commands::open_external_link,
            commands::write_terminal,
            commands::resize_terminal,
            commands::restart_container,
            commands::restart_tmux,
            commands::clipboard_write,
            commands::clipboard_read,
            commands::load_targets_config,
            commands::update_targets_config,
            commands::load_app_settings,
            commands::save_app_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building univers-ark-developer")
        .run(|app_handle, event| {
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                let tunnel_state = app_handle.state::<TunnelState>();
                let dashboard_state = app_handle.state::<DashboardState>();
                stop_all_tunnels(tunnel_state.inner());
                stop_all_dashboard_monitors(dashboard_state.inner());
            }
        });
}
