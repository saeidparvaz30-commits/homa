#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use homa_lib::{model::AgentState, overlay, poller};

#[tauri::command]
fn get_agents(state: tauri::State<poller::Shared>) -> Vec<AgentState> {
    state.lock().unwrap().clone()
}

#[tauri::command]
fn set_alias(session_id: String, name: String) -> Result<(), String> {
    homa_lib::alias::set_session_in(&homa_lib::alias::aliases_path(), &session_id, &name)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_aliases() -> homa_lib::alias::Aliases {
    homa_lib::alias::load()
}

#[tauri::command]
fn hide_overlay(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn focus_session(pid: u32, cwd: String, session_id: String) -> Result<(), String> {
    let tail = homa_lib::limit::read_tail(
        &homa_lib::poller::transcript_path(&cwd, &session_id),
        65_536,
    );
    let title = homa_lib::limit::last_ai_title(&tail);
    match homa_lib::terminal::resolve_hwnd(pid, title.as_deref()) {
        Some(h) => {
            homa_lib::terminal::focus_hwnd(h);
            Ok(())
        }
        None => Err("no terminal window found".into()),
    }
}

/// Left-clicking the tray toggles the overlay roster, Homa's primary surface.
/// The v1 dashboard window stays reachable via the tray menu's "Show Homa".
fn toggle_overlay(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
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
        .invoke_handler(tauri::generate_handler![
            get_agents,
            set_alias,
            get_aliases,
            hide_overlay,
            focus_session
        ])
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Show Homa", true, None::<&str>)?;
            let show_overlay =
                MenuItem::with_id(app, "show-overlay", "Show overlay", true, None::<&str>)?;
            let auto_label = if homa_lib::settings::Settings::load().auto_resume_enabled {
                "Auto-resume: on"
            } else {
                "Auto-resume: off"
            };
            let autoresume = MenuItem::with_id(app, "autoresume", auto_label, true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &show_overlay, &autoresume, &quit])?;
            let auto_item = autoresume.clone();

            let icon = tauri::image::Image::from_path(homa_lib::tray::icon_path(
                app.handle(),
                "tray-ended.png",
            ))?;

            TrayIconBuilder::with_id("homa-tray")
                .icon(icon)
                .tooltip("Homa")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "show-overlay" => {
                        if let Some(w) = app.get_webview_window("overlay") {
                            let _ = w.show();
                        }
                    }
                    "autoresume" => {
                        let mut s = homa_lib::settings::Settings::load();
                        s.auto_resume_enabled = !s.auto_resume_enabled;
                        s.save();
                        let _ = auto_item.set_text(if s.auto_resume_enabled {
                            "Auto-resume: on"
                        } else {
                            "Auto-resume: off"
                        });
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
                        toggle_overlay(tray.app_handle());
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

            if let Some(w) = app.get_webview_window("overlay") {
                w.on_window_event(move |e| {
                    if let WindowEvent::Moved(pos) = e {
                        overlay::remember_position(pos.x as f64, pos.y as f64);
                    }
                    if let WindowEvent::Resized(size) = e {
                        overlay::remember_size(size.width as f64, size.height as f64);
                    }
                });
            }
            overlay::restore_and_show(&app.handle().clone());

            poller::start_watching(app.handle().clone(), shared_setup.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Homa");
}
