// Copyright 2026 Philip Bonev
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://apache.org
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod about;
mod files;
mod info;
mod launch;
mod paths;
mod permissions;
mod search;
mod settings;
mod state;
mod transfer;
mod viewer;
mod windows;

use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Manager;

use crate::state::{AppState, WindowGeometry};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            geometry: Mutex::new(WindowGeometry::default()),
            cancel_flags: Mutex::new(HashMap::new()),
            progress: Mutex::new(HashMap::new()),
        })
        .setup(|app| {
            let settings = settings::load_settings();
            if let Some(window) = app.get_webview_window("main") {
                if let (Some(w), Some(h)) = (settings.width, settings.height) {
                    let _ = window.set_size(tauri::PhysicalSize::new(w, h));
                }
                if let (Some(x), Some(y)) = (settings.x, settings.y) {
                    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            let is_main = window.label() == "main";
            match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    if is_main {
                        // Closing the main window closes the whole app:
                        // close every secondary window as well.
                        let handle = window.app_handle().clone();
                        for (label, w) in handle.webview_windows() {
                            if label != "main" {
                                let _ = w.close();
                            }
                        }
                    }
                    // Secondary windows: close only themselves; the app keeps
                    // running while "main" is open.
                }
                tauri::WindowEvent::Resized(size) if is_main => {
                    let state = window.app_handle().state::<AppState>();
                    let mut geo = state.geometry.lock().unwrap();
                    geo.size = Some((size.width, size.height));
                }
                tauri::WindowEvent::Moved(position) if is_main => {
                    let state = window.app_handle().state::<AppState>();
                    let mut geo = state.geometry.lock().unwrap();
                    geo.position = Some((position.x, position.y));
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            files::list_dir,
            paths::home_dir,
            files::make_dir,
            files::rename_path,
            search::search_files,
            files::delete_path,
            transfer::delete_path_progress,
            transfer::copy_path,
            transfer::move_path,
            transfer::link_path,
            transfer::copy_path_progress,
            transfer::move_path_progress,
            transfer::cancel_copy,
            transfer::get_copy_progress,
            about::get_app_version,
            about::get_about_info,
            viewer::read_text_file,
            viewer::read_file_chunk,
            viewer::authorize_view_image,
            viewer::highlight_code_text,
            info::path_info,
            launch::open_path,
            launch::edit_path,
            launch::run_diff,
            launch::run_edit,
            launch::run_command,
            windows::quit_app,
            windows::open_settings,
            windows::open_help,
            windows::open_search,
            windows::open_view,
            windows::open_info,
            windows::open_permissions,
            permissions::get_file_permissions,
            permissions::set_file_permissions,
            settings::get_appearance,
            settings::set_theme,
            settings::set_font,
            settings::set_font_size,
            settings::set_diff_command,
            settings::set_diff_in_terminal,
            settings::set_edit_command,
            settings::set_edit_in_terminal,
            settings::set_show_hidden,
            settings::set_fuzzy_search,
            settings::set_column_widths,
            settings::get_favorites,
            settings::set_favorites,
            settings::get_fav_apps,
            settings::set_fav_apps,
            paths::expand_env
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app: &tauri::AppHandle, event: tauri::RunEvent| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                // code is None when the exit is triggered by closing a window.
                // If main is still open, a secondary window closed — keep the app running.
                if code.is_none() && app.get_webview_window("main").is_some() {
                    api.prevent_exit();
                    return;
                }
                windows::save_window_geometry(app);
            }
        });
}
