use crate::{
    cleanup::cleanup_stale_ssh_tunnels,
    commands,
    models::{TerminalState, TunnelState},
    tunnel::stop_all_tunnels,
};
use tauri::Manager;

pub(crate) fn run() {
    tauri::Builder::default()
        .manage(TerminalState::default())
        .manage(TunnelState::default())
        .setup(|_| {
            match cleanup_stale_ssh_tunnels() {
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
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_bootstrap,
            commands::refresh_bootstrap,
            commands::load_server_inventory,
            commands::refresh_server_inventory,
            commands::attach_terminal,
            commands::ensure_tunnel,
            commands::restart_tunnel,
            commands::write_terminal,
            commands::resize_terminal
        ])
        .build(tauri::generate_context!())
        .expect("error while building univers-ark-developer")
        .run(|app_handle, event| {
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                let tunnel_state = app_handle.state::<TunnelState>();
                stop_all_tunnels(tunnel_state.inner());
            }
        });
}
