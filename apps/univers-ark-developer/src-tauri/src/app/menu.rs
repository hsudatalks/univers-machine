use tauri::{
    AppHandle, Emitter, Runtime,
    menu::{
        AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu,
        WINDOW_SUBMENU_ID,
    },
};

const TOGGLE_SIDEBAR_MENU_ID: &str = "toggle_sidebar";
const TOGGLE_SIDEBAR_EVENT: &str = "toggle-sidebar-requested";
const PREVIOUS_CONTAINER_MENU_ID: &str = "previous_container";
const NEXT_CONTAINER_MENU_ID: &str = "next_container";
const PARENT_VIEW_MENU_ID: &str = "parent_view";
const PREVIOUS_CONTAINER_EVENT: &str = "previous-container-requested";
const NEXT_CONTAINER_EVENT: &str = "next-container-requested";
const PARENT_VIEW_EVENT: &str = "parent-view-requested";

#[cfg(target_os = "macos")]
pub(super) fn build_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
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

#[cfg(not(target_os = "macos"))]
pub(super) fn build_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
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

pub(super) fn handle_menu_event<R: Runtime>(app_handle: &AppHandle<R>, menu_id: &str) {
    if menu_id == TOGGLE_SIDEBAR_MENU_ID {
        let _ = app_handle.emit(TOGGLE_SIDEBAR_EVENT, ());
    } else if menu_id == PREVIOUS_CONTAINER_MENU_ID {
        let _ = app_handle.emit(PREVIOUS_CONTAINER_EVENT, ());
    } else if menu_id == NEXT_CONTAINER_MENU_ID {
        let _ = app_handle.emit(NEXT_CONTAINER_EVENT, ());
    } else if menu_id == PARENT_VIEW_MENU_ID {
        let _ = app_handle.emit(PARENT_VIEW_EVENT, ());
    }
}
