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

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Emitter;

use crate::paths::home_dir_path;

fn default_show_hidden() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ColumnWidths {
    name: f64,
    ext: f64,
    size: f64,
    date: f64,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        ColumnWidths {
            name: 0.0,
            ext: 60.0,
            size: 80.0,
            date: 120.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct AppSettings {
    pub(crate) theme: String,
    pub(crate) font: String,
    pub(crate) font_size: u32,
    pub(crate) diff_command: String,
    pub(crate) diff_in_terminal: bool,
    pub(crate) edit_command: String,
    pub(crate) edit_in_terminal: bool,
    pub(crate) favorites: Vec<String>,
    pub(crate) fav_apps: Vec<String>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) x: Option<i32>,
    pub(crate) y: Option<i32>,
    #[serde(default = "default_show_hidden")]
    pub(crate) show_hidden: bool,
    #[serde(default)]
    pub(crate) fuzzy_search: bool,
    #[serde(default)]
    pub(crate) column_widths: ColumnWidths,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            theme: "system".to_string(),
            font: "default".to_string(),
            font_size: 14,
            diff_command: String::new(),
            diff_in_terminal: false,
            edit_command: String::new(),
            edit_in_terminal: false,
            favorites: Vec::new(),
            fav_apps: Vec::new(),
            width: None,
            height: None,
            x: None,
            y: None,
            show_hidden: true,
            fuzzy_search: false,
            column_widths: ColumnWidths::default(),
        }
    }
}

fn settings_path() -> PathBuf {
    let home = home_dir_path().unwrap_or_else(|| PathBuf::from("."));
    home.join(".p2manager").join("config.json")
}

pub(crate) fn load_settings() -> AppSettings {
    let path = settings_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

pub(crate) fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Error writing settings: {}", e))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Error writing settings: {}", e))
}

#[tauri::command]
pub(crate) fn get_appearance() -> Result<AppSettings, String> {
    Ok(load_settings())
}

#[tauri::command]
pub(crate) fn set_theme(theme: String, app: tauri::AppHandle) -> Result<AppSettings, String> {
    if !["system", "light", "dark"].contains(&theme.as_str()) {
        return Err("Invalid theme.".to_string());
    }
    let mut settings = load_settings();
    settings.theme = theme;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_font(font: String, app: tauri::AppHandle) -> Result<AppSettings, String> {
    if !["default", "monospace", "serif", "sans"].contains(&font.as_str()) {
        return Err("Invalid font.".to_string());
    }
    let mut settings = load_settings();
    settings.font = font;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_font_size(font_size: u32, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.font_size = font_size.clamp(10, 28);
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_diff_command(
    command: String,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.diff_command = command;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_diff_in_terminal(
    in_terminal: bool,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.diff_in_terminal = in_terminal;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn get_favorites() -> Result<Vec<String>, String> {
    Ok(load_settings().favorites)
}

#[tauri::command]
pub(crate) fn set_favorites(
    favorites: Vec<String>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let mut settings = load_settings();
    settings.favorites = favorites;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings.favorites)
}

#[tauri::command]
pub(crate) fn get_fav_apps() -> Result<Vec<String>, String> {
    Ok(load_settings().fav_apps)
}

#[tauri::command]
pub(crate) fn set_fav_apps(
    fav_apps: Vec<String>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let mut settings = load_settings();
    settings.fav_apps = fav_apps;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings.fav_apps)
}

#[tauri::command]
pub(crate) fn set_edit_command(
    command: String,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.edit_command = command;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_edit_in_terminal(
    in_terminal: bool,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.edit_in_terminal = in_terminal;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_show_hidden(
    show_hidden: bool,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.show_hidden = show_hidden;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_fuzzy_search(
    fuzzy_search: bool,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.fuzzy_search = fuzzy_search;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
pub(crate) fn set_column_widths(
    column_widths: ColumnWidths,
    _app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.column_widths = column_widths;
    save_settings(&settings)?;
    Ok(settings)
}
