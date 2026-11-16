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

use tauri::webview::WebviewWindowBuilder;
use tauri::{Manager, PhysicalPosition, WebviewUrl};

use crate::settings::{load_settings, save_settings};
use crate::state::AppState;

#[tauri::command]
pub(crate) fn quit_app(app: tauri::AppHandle) {
    save_window_geometry(&app);
    app.exit(0);
}

pub(crate) fn save_window_geometry(app: &tauri::AppHandle) {
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => return,
    };

    let state = app.state::<AppState>();
    let geometry = state.geometry.lock().unwrap();
    let size = window
        .inner_size()
        .ok()
        .map(|size| (size.width, size.height))
        .or(geometry.size);
    let position = window
        .outer_position()
        .ok()
        .map(|position| (position.x, position.y))
        .or(geometry.position);
    drop(geometry);

    let mut settings = load_settings();
    if let Some((width, height)) = size {
        settings.width = Some(width);
        settings.height = Some(height);
    }
    if let Some((x, y)) = position {
        settings.x = Some(x);
        settings.y = Some(y);
    }
    let _ = save_settings(&settings);
}

struct ChildWindowSpec {
    label: &'static str,
    url: &'static str,
    title: &'static str,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
}

fn open_child_window(app: &tauri::AppHandle, spec: &ChildWindowSpec) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(spec.label) {
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found.".to_string())?;
    let scale = main.scale_factor().unwrap_or(1.0);
    let pos = main.outer_position().map_err(|e| e.to_string())?;
    let size = main.outer_size().map_err(|e| e.to_string())?;
    let child_w = (spec.width * scale).round() as u32;
    let child_h = (spec.height * scale).round() as u32;
    let mut x = pos.x + (size.width as i32 - child_w as i32) / 2;
    let mut y = pos.y + (size.height as i32 - child_h as i32) / 2;
    if let Ok(Some(monitor)) = main.current_monitor() {
        let mp = monitor.position();
        let ms = monitor.size();
        let max_x = mp.x + ms.width as i32 - child_w as i32;
        let max_y = mp.y + ms.height as i32 - child_h as i32;
        x = x.clamp(mp.x, max_x.max(mp.x));
        y = y.clamp(mp.y, max_y.max(mp.y));
    }
    let window = WebviewWindowBuilder::new(app, spec.label, WebviewUrl::App(spec.url.into()))
        .title(spec.title)
        .inner_size(spec.width, spec.height)
        .min_inner_size(spec.min_width, spec.min_height)
        .build()
        .map_err(|e| e.to_string())?;
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
pub(crate) async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    open_child_window(
        &app,
        &ChildWindowSpec {
            label: "settings",
            url: "settings.html",
            title: "Settings",
            width: 560.0,
            height: 680.0,
            min_width: 460.0,
            min_height: 520.0,
        },
    )
}

#[tauri::command]
pub(crate) async fn open_help(app: tauri::AppHandle) -> Result<(), String> {
    open_child_window(
        &app,
        &ChildWindowSpec {
            label: "help",
            url: "help.html",
            title: "Help",
            width: 840.0,
            height: 620.0,
            min_width: 560.0,
            min_height: 420.0,
        },
    )
}

#[tauri::command]
pub(crate) async fn open_search(app: tauri::AppHandle) -> Result<(), String> {
    open_child_window(
        &app,
        &ChildWindowSpec {
            label: "search",
            url: "search.html",
            title: "Search files",
            width: 760.0,
            height: 660.0,
            min_width: 640.0,
            min_height: 560.0,
        },
    )
}

#[tauri::command]
pub(crate) async fn open_view(app: tauri::AppHandle) -> Result<(), String> {
    open_child_window(
        &app,
        &ChildWindowSpec {
            label: "view",
            url: "view.html",
            title: "View",
            width: 900.0,
            height: 700.0,
            min_width: 560.0,
            min_height: 400.0,
        },
    )
}

#[tauri::command]
pub(crate) async fn open_info(app: tauri::AppHandle) -> Result<(), String> {
    open_child_window(
        &app,
        &ChildWindowSpec {
            label: "info",
            url: "info.html",
            title: "Info",
            width: 520.0,
            height: 460.0,
            min_width: 420.0,
            min_height: 320.0,
        },
    )
}

#[tauri::command]
pub(crate) async fn open_permissions(app: tauri::AppHandle) -> Result<(), String> {
    open_child_window(
        &app,
        &ChildWindowSpec {
            label: "permissions",
            url: "permissions.html",
            title: "Permissions",
            width: 520.0,
            height: 440.0,
            min_width: 440.0,
            min_height: 360.0,
        },
    )
}
