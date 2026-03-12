use crate::{
    cleanup::cleanup_stale_ssh_tunnels,
    commands,
    machine::initialize_targets_file_path,
    models::{
        ConnectivityState, DashboardState, RuntimeActivityState, SchedulerState, ServiceState,
        TerminalState, TunnelState,
    },
    runtime::{
        dashboard::stop_all_dashboard_monitors,
        scheduler::{start_background_scheduler, stop_background_scheduler},
        tunnel::stop_all_tunnels,
    },
    secrets::SecretManagementState,
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
const PREVIOUS_CONTAINER_MENU_ID: &str = "previous_container";
const NEXT_CONTAINER_MENU_ID: &str = "next_container";
const PARENT_VIEW_MENU_ID: &str = "parent_view";
const PREVIOUS_CONTAINER_EVENT: &str = "previous-container-requested";
const NEXT_CONTAINER_EVENT: &str = "next-container-requested";
const PARENT_VIEW_EVENT: &str = "parent-view-requested";

#[cfg(all(desktop, target_os = "macos"))]
fn build_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let pkg_info = app_handle.package_info();
    let config = app_handle.config();
    let about_metadata = AboutMetadata {
        name: Some(pkg_info.name.clone()),
        version: Some(pkg_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };

    let toggle_sidebar = MenuItem::with_id(
        app_handle,
        TOGGLE_SIDEBAR_MENU_ID,
        "Toggle Sidebar Menu",
        true,
        Some("Cmd+H"),
    )?;
    let previous_container = MenuItem::with_id(
        app_handle,
        PREVIOUS_CONTAINER_MENU_ID,
        "Previous Container",
        true,
        Some("Cmd+Alt+Left"),
    )?;
    let next_container = MenuItem::with_id(
        app_handle,
        NEXT_CONTAINER_MENU_ID,
        "Next Container",
        true,
        Some("Cmd+Alt+Right"),
    )?;
    let parent_view = MenuItem::with_id(
        app_handle,
        PARENT_VIEW_MENU_ID,
        "Parent View",
        true,
        Some("Cmd+Alt+Up"),
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
        &[
            &previous_container,
            &next_container,
            &parent_view,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::fullscreen(app_handle, None)?,
        ],
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

    let help_menu = Submenu::with_id_and_items(app_handle, HELP_SUBMENU_ID, "Help", true, &[])?;

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

#[cfg(all(desktop, not(target_os = "macos")))]
fn build_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let toggle_sidebar = MenuItem::with_id(
        app_handle,
        TOGGLE_SIDEBAR_MENU_ID,
        "Toggle Sidebar",
        true,
        Some("Ctrl+H"),
    )?;
    let previous_container = MenuItem::with_id(
        app_handle,
        PREVIOUS_CONTAINER_MENU_ID,
        "Previous Container",
        true,
        Some("Ctrl+Alt+Left"),
    )?;
    let next_container = MenuItem::with_id(
        app_handle,
        NEXT_CONTAINER_MENU_ID,
        "Next Container",
        true,
        Some("Ctrl+Alt+Right"),
    )?;
    let parent_view = MenuItem::with_id(
        app_handle,
        PARENT_VIEW_MENU_ID,
        "Parent View",
        true,
        Some("Ctrl+Alt+Up"),
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
        &[
            &previous_container,
            &next_container,
            &parent_view,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::fullscreen(app_handle, None)?,
        ],
    )?;

    Menu::with_items(app_handle, &[&file_menu, &view_menu])
}

#[cfg(desktop)]
fn apply_platform_features<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .menu(build_app_menu)
        .on_menu_event(|app_handle, event| {
            if event.id().as_ref() == TOGGLE_SIDEBAR_MENU_ID {
                let _ = app_handle.emit(TOGGLE_SIDEBAR_EVENT, ());
            } else if event.id().as_ref() == PREVIOUS_CONTAINER_MENU_ID {
                let _ = app_handle.emit(PREVIOUS_CONTAINER_EVENT, ());
            } else if event.id().as_ref() == NEXT_CONTAINER_MENU_ID {
                let _ = app_handle.emit(NEXT_CONTAINER_EVENT, ());
            } else if event.id().as_ref() == PARENT_VIEW_MENU_ID {
                let _ = app_handle.emit(PARENT_VIEW_EVENT, ());
            }
        })
}

#[cfg(mobile)]
fn apply_platform_features<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
}

pub(crate) fn run() {
    let builder = tauri::Builder::default()
        .manage(TerminalState::default())
        .manage(TunnelState::default())
        .manage(ServiceState::default())
        .manage(DashboardState::default())
        .manage(ConnectivityState::default())
        .manage(RuntimeActivityState::default())
        .manage(SchedulerState::default())
        .manage(SecretManagementState::new().expect("failed to initialize secret management"))
        // NOTE: Keyboard shortcuts for container navigation (Ctrl+Alt+Left/Right/Up
        // on Windows, Cmd+Alt+Left/Right/Up on macOS) are handled by the menu
        // accelerators in build_app_menu, not by global shortcuts.
        ;

    apply_platform_features(builder)
        .setup(|app| {
            initialize_targets_file_path(app.handle())?;
            start_background_scheduler(
                app.handle().clone(),
                app.state::<SchedulerState>().inner().clone(),
                app.state::<TunnelState>().inner().clone(),
                app.state::<ConnectivityState>().inner().clone(),
                app.state::<DashboardState>().inner().clone(),
                app.state::<RuntimeActivityState>().inner().clone(),
            );

            #[cfg(desktop)]
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
            commands::machine::load_bootstrap,
            commands::machine::refresh_bootstrap,
            commands::machine::load_machine_inventory,
            commands::machine::refresh_machine_inventory,
            commands::machine::scan_machine_inventory,
            commands::machine::scan_ssh_config_machine_candidates,
            commands::machine::scan_tailscale_machine_candidates,
            commands::runtime::attach_terminal,
            commands::runtime::restart_terminal,
            commands::runtime::ensure_tunnel,
            commands::runtime::sync_tunnel_registrations,
            commands::runtime::restart_tunnel,
            commands::runtime::restart_all_tunnels,
            commands::runtime::list_remote_directory,
            commands::runtime::read_remote_file_preview,
            commands::runtime::load_container_dashboard,
            commands::runtime::start_dashboard_monitor,
            commands::runtime::stop_dashboard_monitor,
            commands::runtime::refresh_container_dashboard,
            commands::github::load_github_project_state,
            commands::github::load_github_pull_request_detail,
            commands::github::merge_github_pull_request,
            commands::github::open_external_link,
            commands::runtime::write_terminal,
            commands::runtime::resize_terminal,
            commands::machine::restart_container,
            commands::runtime::restart_tmux,
            commands::runtime::execute_command_service,
            commands::runtime::clipboard_write,
            commands::runtime::clipboard_read,
            commands::machine::load_targets_config,
            commands::machine::update_targets_config,
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
            commands::runtime::update_runtime_activity
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
                let scheduler_state = app_handle.state::<SchedulerState>();
                stop_background_scheduler(scheduler_state.inner());
                stop_all_tunnels(tunnel_state.inner());
                stop_all_dashboard_monitors(dashboard_state.inner());
            }
        });
}
