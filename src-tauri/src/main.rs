#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use homa_lib::{model::AgentState, poller};

#[tauri::command]
fn get_agents(state: tauri::State<poller::Shared>) -> Vec<AgentState> {
    state.lock().unwrap().clone()
}

#[tauri::command]
fn set_alias(cwd: String, name: String) -> Result<(), String> {
    homa_lib::alias::set_in(&homa_lib::alias::aliases_path(), &cwd, &name)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_aliases() -> homa_lib::alias::Aliases {
    homa_lib::alias::load()
}

fn toggle_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

fn main() {
    let shared: poller::Shared = Arc::new(Mutex::new(Vec::new()));
    let shared_setup = shared.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(shared)
        .invoke_handler(tauri::generate_handler![get_agents, set_alias, get_aliases])
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Show Homa", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let icon = tauri::image::Image::from_path(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("icons")
                    .join("tray-ended.png"),
            )?;

            TrayIconBuilder::with_id("homa-tray")
                .icon(icon)
                .tooltip("Homa")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_main(tray.app_handle());
                    }
                })
                .build(app)?;

            // Closing the main window hides it instead of quitting: stay in the tray.
            if let Some(w) = app.get_webview_window("main") {
                let wc = w.clone();
                w.on_window_event(move |e| {
                    if let WindowEvent::CloseRequested { api, .. } = e {
                        api.prevent_close();
                        let _ = wc.hide();
                    }
                });
            }

            // Register autostart only in release builds (avoid pinning a dev binary).
            #[cfg(not(debug_assertions))]
            {
                use tauri_plugin_autostart::ManagerExt;
                let _ = app.autolaunch().enable();
            }

            poller::start_watching(app.handle().clone(), shared_setup.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Homa");
}
