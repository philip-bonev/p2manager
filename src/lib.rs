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
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;
use tauri::{Emitter, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    name: String,
    is_dir: bool,
    size: u64,
    modified: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirListing {
    path: String,
    parent: Option<String>,
    items: Vec<FileEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileInfo {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: u64,
    created: u64,
    permissions: String,
}

fn home_dir_path() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var(key).ok().map(PathBuf::from)
}

fn default_show_hidden() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
struct ColumnWidths {
    name: f64,
    ext: f64,
    size: f64,
    date: f64,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        ColumnWidths { name: 0.0, ext: 60.0, size: 80.0, date: 120.0 }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
struct AppSettings {
    theme: String,
    font: String,
    font_size: u32,
    diff_command: String,
    diff_in_terminal: bool,
    edit_command: String,
    edit_in_terminal: bool,
    favorites: Vec<String>,
    fav_apps: Vec<String>,
    width: Option<u32>,
    height: Option<u32>,
    x: Option<i32>,
    y: Option<i32>,
    #[serde(default = "default_show_hidden")]
    show_hidden: bool,
    #[serde(default)]
    fuzzy_search: bool,
    #[serde(default)]
    column_widths: ColumnWidths,
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

#[derive(Default)]
struct WindowGeometry {
    size: Option<(u32, u32)>,
    position: Option<(i32, i32)>,
}

struct AppState {
    geometry: Mutex<WindowGeometry>,
    cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    progress: Mutex<HashMap<String, CopyProgress>>,
}

fn settings_path() -> PathBuf {
    let home = home_dir_path().unwrap_or_else(|| PathBuf::from("."));
    home.join(".p2manager").join("config.json")
}

fn load_settings() -> AppSettings {
    let path = settings_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Error writing settings: {}", e))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Error writing settings: {}", e))
}

fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn entry_from_path(path: &Path) -> Result<FileEntry, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Error reading {}: {}", path.display(), e))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    Ok(FileEntry {
        name,
        is_dir: meta.is_dir(),
        size: meta.len(),
        modified: modified_secs(&meta),
    })
}

fn is_hidden(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = fs::metadata(path) {
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            if meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
                return true;
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    }
}

fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("Error creating {}: {}", dst.display(), e))?;
        for entry in fs::read_dir(src).map_err(|e| format!("Error reading {}: {}", src.display(), e))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let child_dst = dst.join(entry.file_name());
            copy_recursive(&entry.path(), &child_dst)?;
        }
    } else {
        fs::copy(src, dst)
            .map_err(|e| format!("Error copying {}: {}", src.display(), e))?;
    }
    Ok(())
}

fn is_descendant(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParams {
    base_path: String,
    exclusions: String,
    pattern: String,
    pattern_mode: String,
    ignore_case: bool,
    recursive: bool,
    content_enabled: bool,
    content_pattern: String,
    content_mode: String,
    content_ignore_case: bool,
}

fn glob_to_regex(pattern: &str) -> String {
    let mut re = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
    }
    re.push('$');
    re
}

fn build_file_matcher(pattern: &str, mode: &str, ignore_case: bool) -> Result<Option<regex::Regex>, String> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return Ok(None);
    }
    let re_str = if mode == "regexp" {
        pat.to_string()
    } else {
        glob_to_regex(pat)
    };
    let mut builder = regex::RegexBuilder::new(&re_str);
    builder.case_insensitive(ignore_case);
    builder
        .build()
        .map(Some)
        .map_err(|e| format!("Invalid pattern: {}", e))
}

fn build_content_matcher(
    pattern: &str,
    mode: &str,
    ignore_case: bool,
) -> Result<(Option<regex::Regex>, Option<String>), String> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return Ok((None, None));
    }
    if mode == "regexp" {
        let mut builder = regex::RegexBuilder::new(pat);
        builder.case_insensitive(ignore_case);
        let re = builder.build().map_err(|e| format!("Invalid content pattern: {}", e))?;
        Ok((Some(re), None))
    } else {
        let plain = if ignore_case { pat.to_lowercase() } else { pat.to_string() };
        Ok((None, Some(plain)))
    }
}

fn file_content_matches(
    path: &Path,
    content_re: &Option<regex::Regex>,
    content_plain: &Option<String>,
    content_ignore_case: bool,
) -> bool {
    if content_re.is_none() && content_plain.is_none() {
        return true;
    }
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if data.len() > 20 * 1024 * 1024 {
        return false;
    }
    let text = String::from_utf8_lossy(&data);
    if let Some(re) = content_re {
        return re.is_match(&text);
    }
    if let Some(plain) = content_plain {
        if content_ignore_case {
            return text.to_lowercase().contains(plain);
        } else {
            return text.contains(plain);
        }
    }
    true
}

fn search_walk(
    dir: &Path,
    exclusions: &[String],
    file_re: &Option<regex::Regex>,
    content_re: &Option<regex::Regex>,
    content_plain: &Option<String>,
    content_enabled: bool,
    content_ignore_case: bool,
    recursive: bool,
    results: &mut Vec<String>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let excluded = exclusions.iter().any(|e| e == name);
        if excluded {
            continue;
        }
        let name_matches = match file_re {
            Some(re) => re.is_match(name),
            None => true,
        };
        let is_dir = p.is_dir();
        if content_enabled {
            if name_matches {
                if is_dir {
                    if recursive {
                        search_walk(
                            &p,
                            exclusions,
                            file_re,
                            content_re,
                            content_plain,
                            content_enabled,
                            content_ignore_case,
                            recursive,
                            results,
                        );
                    }
                } else if file_content_matches(&p, content_re, content_plain, content_ignore_case) {
                    results.push(p.to_string_lossy().to_string());
                }
            } else if is_dir && recursive {
                search_walk(
                    &p,
                    exclusions,
                    file_re,
                    content_re,
                    content_plain,
                    content_enabled,
                    content_ignore_case,
                    recursive,
                    results,
                );
            }
        } else {
            if name_matches {
                results.push(p.to_string_lossy().to_string());
            }
            if is_dir && recursive {
                search_walk(
                    &p,
                    exclusions,
                    file_re,
                    content_re,
                    content_plain,
                    content_enabled,
                    content_ignore_case,
                    recursive,
                    results,
                );
            }
        }
    }
}

fn resolve_dest(dst_dir: &Path, name: &str) -> PathBuf {
    let mut candidate = dst_dir.join(name);
    let mut n = 0;
    while candidate.exists() {
        n += 1;
        candidate = dst_dir.join(format!("{}_{}", name, n));
    }
    candidate
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CopyProgress {
    id: String,
    copied: u64,
    total: u64,
    path: String,
}

fn copy_file_progress(
    src: &Path,
    dst: &Path,
    app: &tauri::AppHandle,
    id: &str,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let total = fs::metadata(src)
        .map_err(|e| format!("Error reading {}: {}", src.display(), e))?
        .len();
    let mut src_file = fs::File::open(src)
        .map_err(|e| format!("Error opening {}: {}", src.display(), e))?;
    let mut dst_file = fs::File::create(dst)
        .map_err(|e| format!("Error creating {}: {}", dst.display(), e))?;
    let mut buf = vec![0u8; 1 << 20];
    let mut copied: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(dst_file);
            let _ = fs::remove_file(dst);
            return Err("CANCELLED".to_string());
        }
        let n = src_file
            .read(&mut buf)
            .map_err(|e| format!("Error reading {}: {}", src.display(), e))?;
        if n == 0 {
            break;
        }
        dst_file
            .write_all(&buf[..n])
            .map_err(|e| format!("Error writing to {}: {}", dst.display(), e))?;
        copied += n as u64;
        if let Some(state) = app.try_state::<AppState>() {
            state.progress.lock().unwrap().insert(
                id.to_string(),
                CopyProgress {
                    id: id.to_string(),
                    copied,
                    total,
                    path: src.to_string_lossy().to_string(),
                },
            );
        }
    }
    Ok(())
}

fn copy_recursive_progress(
    src: &Path,
    dst: &Path,
    app: &tauri::AppHandle,
    id: &str,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("CANCELLED".to_string());
    }
    if src.is_dir() {
        fs::create_dir_all(dst)
            .map_err(|e| format!("Error creating {}: {}", dst.display(), e))?;
        for entry in
            fs::read_dir(src).map_err(|e| format!("Error reading {}: {}", src.display(), e))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_recursive_progress(&entry.path(), &dst.join(entry.file_name()), app, id, cancel)?;
        }
    } else {
        copy_file_progress(src, dst, app, id, cancel)?;
    }
    Ok(())
}

fn register_cancel(app: &tauri::AppHandle, id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Some(state) = app.try_state::<AppState>() {
        state.cancel_flags.lock().unwrap().insert(id.to_string(), flag.clone());
    }
    flag
}

fn unregister_cancel(app: &tauri::AppHandle, id: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.cancel_flags.lock().unwrap().remove(id);
    }
}

fn unregister_progress(app: &tauri::AppHandle, id: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.progress.lock().unwrap().remove(id);
    }
}

fn cleanup_partial(dest: &Path, is_dir: bool) {
    let _ = if is_dir {
        fs::remove_dir_all(dest)
    } else {
        fs::remove_file(dest)
    };
}

#[tauri::command]
fn list_dir(path: String) -> Result<DirListing, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }
    let show_hidden = load_settings().show_hidden;
    let mut items = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("Error reading {}: {}", path, e))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        if !show_hidden && is_hidden(&p) {
            continue;
        }
        match entry_from_path(&p) {
            Ok(fe) => items.push(fe),
            Err(_) => continue,
        }
    }
    items.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let parent = dir
        .parent()
        .map(|p| p.to_string_lossy().to_string());
    Ok(DirListing {
        path: dir.to_string_lossy().to_string(),
        parent,
        items,
    })
}

#[tauri::command]
async fn search_files(params: SearchParams) -> Result<Vec<String>, String> {
    let base = PathBuf::from(params.base_path.clone());
    if !base.is_dir() {
        return Err(format!("Not a directory: {}", params.base_path));
    }
    let exclusions: Vec<String> = params
        .exclusions
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let file_re = build_file_matcher(&params.pattern, &params.pattern_mode, params.ignore_case)?;
    let (content_re, content_plain) = if params.content_enabled {
        build_content_matcher(
            &params.content_pattern,
            &params.content_mode,
            params.content_ignore_case,
        )?
    } else {
        (None, None)
    };
    let recursive = params.recursive;
    let content_enabled = params.content_enabled;
    let content_ignore_case = params.content_ignore_case;
    let res = tauri::async_runtime::spawn_blocking(move || {
        let mut results = Vec::new();
        search_walk(
            &base,
            &exclusions,
            &file_re,
            &content_re,
            &content_plain,
            content_enabled,
            content_ignore_case,
            recursive,
            &mut results,
        );
        results.sort();
        results
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(res)
}

#[tauri::command]
fn home_dir() -> Result<String, String> {
    home_dir_path()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Cannot determine home directory.".to_string())
}

#[tauri::command]
fn make_dir(parent: String, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Folder name cannot be empty.".to_string());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("Name contains invalid characters.".to_string());
    }
    let path = PathBuf::from(&parent).join(name);
    if path.exists() {
        return Err(format!("\"{}\" already exists.", name));
    }
    fs::create_dir(&path)
        .map_err(|e| format!("Error creating folder \"{}\": {}", name, e))?;
    Ok(())
}

#[tauri::command]
fn rename_path(path: String, new_name: String) -> Result<String, String> {
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("Name cannot be empty.".to_string());
    }
    if new_name.contains('/') || new_name.contains('\\') {
        return Err("Name contains invalid characters.".to_string());
    }
    let src = PathBuf::from(&path);
    if !src.exists() {
        return Err(format!("Does not exist: {}", path));
    }
    let parent = src.parent().ok_or_else(|| "Invalid path.".to_string())?;
    let dest = parent.join(&new_name);
    if dest.exists() {
        return Err(format!("\"{}\" already exists.", new_name));
    }
    fs::rename(&src, &dest).map_err(|e| format!("Error renaming {}: {}", path, e))?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
fn delete_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("Error reading {}: {}", path, e))?;
    let result = if meta.is_dir() {
        fs::remove_dir_all(&p)
    } else {
        fs::remove_file(&p)
    };
    result.map_err(|e| format!("Error deleting {}: {}", path, e))
}

#[tauri::command]
fn copy_path(src: String, dst_dir: String) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst_dir);
    if !dst_path.is_dir() {
        return Err(format!("Not a directory: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Invalid source path.".to_string())?;
    if src_path.parent().map(|p| p == dst_path.as_path()).unwrap_or(false) {
        return Err("File is already in this folder.".to_string());
    }
    if is_descendant(&dst_path, &src_path) {
        return Err("Cannot copy a folder into itself.".to_string());
    }
    let dest = resolve_dest(&dst_path, &name);
    copy_recursive(&src_path, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
fn move_path(src: String, dst_dir: String) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst_dir);
    if !dst_path.is_dir() {
        return Err(format!("Not a directory: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Invalid source path.".to_string())?;
    if src_path.parent().map(|p| p == dst_path.as_path()).unwrap_or(false) {
        return Err("File is already in this folder.".to_string());
    }
    if is_descendant(&dst_path, &src_path) {
        return Err("Cannot move a folder into itself.".to_string());
    }
    let dest = resolve_dest(&dst_path, &name);
    match fs::rename(&src_path, &dest) {
        Ok(_) => Ok(dest.to_string_lossy().to_string()),
        Err(_) => {
            copy_recursive(&src_path, &dest)?;
            let meta = fs::metadata(&src_path).map_err(|e| e.to_string())?;
            let del = if meta.is_dir() {
                fs::remove_dir_all(&src_path)
            } else {
                fs::remove_file(&src_path)
            };
            del.map_err(|e| format!("Copied, but source could not be deleted: {}", e))?;
            Ok(dest.to_string_lossy().to_string())
        }
    }
}

#[tauri::command]
fn link_path(src: String, dst_dir: String, hard: bool) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst_dir);
    if !dst_path.is_dir() {
        return Err(format!("Not a directory: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Invalid source path.".to_string())?;
    let dest = resolve_dest(&dst_path, &name);

    if hard {
        fs::hard_link(&src_path, &dest).map_err(|e| {
            format!("Error creating hardlink of {}: {}", src_path.display(), e)
        })?;
    } else {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&src_path, &dest).map_err(|e| {
                format!("Error creating symlink of {}: {}", src_path.display(), e)
            })?;
        }
        #[cfg(windows)]
        {
            let meta = fs::metadata(&src_path).map_err(|e| {
                format!("Error reading {}: {}", src_path.display(), e)
            })?;
            let result = if meta.is_dir() {
                std::os::windows::fs::symlink_dir(&src_path, &dest)
            } else {
                std::os::windows::fs::symlink_file(&src_path, &dest)
            };
            result.map_err(|e| {
                format!("Error creating symlink of {}: {}", src_path.display(), e)
            })?;
        }
    }
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
async fn copy_path_progress(
    app: tauri::AppHandle,
    src: String,
    dst_dir: String,
    id: String,
) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst_dir);
    if !dst_path.is_dir() {
        return Err(format!("Not a directory: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Invalid source path.".to_string())?;
    if src_path.parent().map(|p| p == dst_path.as_path()).unwrap_or(false) {
        return Err("File is already in this folder.".to_string());
    }
    if is_descendant(&dst_path, &src_path) {
        return Err("Cannot copy a folder into itself.".to_string());
    }
    let dest = resolve_dest(&dst_path, &name);
    let cancel = register_cancel(&app, &id);
    let app2 = app.clone();
    let id2 = id.clone();
    let src2 = src_path.clone();
    let dest2 = dest.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        copy_recursive_progress(&src2, &dest2, &app2, &id2, &cancel)
    })
    .await
    .map_err(|e| format!("Error copying: {}", e))?;
    unregister_cancel(&app, &id);
    unregister_progress(&app, &id);
    match result {
        Ok(_) => Ok(dest.to_string_lossy().to_string()),
        Err(e) => {
            if e == "CANCELLED" {
                cleanup_partial(&dest, src_path.is_dir());
                return Err("CANCELLED".to_string());
            }
            Err(e)
        }
    }
}

#[tauri::command]
async fn move_path_progress(
    app: tauri::AppHandle,
    src: String,
    dst_dir: String,
    id: String,
) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst_dir);
    if !dst_path.is_dir() {
        return Err(format!("Not a directory: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Invalid source path.".to_string())?;
    if src_path.parent().map(|p| p == dst_path.as_path()).unwrap_or(false) {
        return Err("File is already in this folder.".to_string());
    }
    if is_descendant(&dst_path, &src_path) {
        return Err("Cannot move a folder into itself.".to_string());
    }
    let dest = resolve_dest(&dst_path, &name);
    if fs::rename(&src_path, &dest).is_ok() {
        return Ok(dest.to_string_lossy().to_string());
    }
    let cancel = register_cancel(&app, &id);
    let app2 = app.clone();
    let id2 = id.clone();
    let src2 = src_path.clone();
    let dest2 = dest.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        copy_recursive_progress(&src2, &dest2, &app2, &id2, &cancel)
    })
    .await
    .map_err(|e| format!("Error moving: {}", e))?;
    unregister_cancel(&app, &id);
    unregister_progress(&app, &id);
    match result {
        Ok(_) => {
            let meta = fs::metadata(&src_path).map_err(|e| e.to_string())?;
            let del = if meta.is_dir() {
                fs::remove_dir_all(&src_path)
            } else {
                fs::remove_file(&src_path)
            };
            del.map_err(|e| format!("Copied, but source could not be deleted: {}", e))?;
            Ok(dest.to_string_lossy().to_string())
        }
        Err(e) => {
            if e == "CANCELLED" {
                cleanup_partial(&dest, src_path.is_dir());
                return Err("CANCELLED".to_string());
            }
            Err(e)
        }
    }
}

#[tauri::command]
fn cancel_copy(id: String, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Some(flag) = state.cancel_flags.lock().unwrap().get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[tauri::command]
fn get_copy_progress(id: String, app: tauri::AppHandle) -> Result<Option<CopyProgress>, String> {
    Ok(app
        .try_state::<AppState>()
        .and_then(|s| s.progress.lock().unwrap().get(&id).cloned()))
}

#[tauri::command]
fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let data = fs::read(Path::new(&path))
        .map_err(|e| format!("Cannot read file {}: {}", path, e))?;
    let capped = if data.len() > 1_000_000 { &data[..1_000_000] } else { &data[..] };
    Ok(String::from_utf8_lossy(capped).to_string())
}

#[tauri::command]
fn read_file_chunk(path: String, offset: u64, limit: u64) -> Result<String, String> {
    let meta = fs::metadata(Path::new(&path))
        .map_err(|e| format!("Cannot read file {}: {}", path, e))?;
    let file_size = meta.len();
    if offset >= file_size {
        return Ok(String::new());
    }
    let end = std::cmp::min(offset + limit, file_size);
    let len = (end - offset) as usize;
    let mut buf = vec![0u8; len];
    let mut f = fs::File::open(Path::new(&path))
        .map_err(|e| format!("Cannot open file {}: {}", path, e))?;
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Error seeking: {}", e))?;
    f.read_exact(&mut buf)
        .map_err(|e| format!("Error reading: {}", e))?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

#[tauri::command]
fn path_info(path: String) -> Result<FileInfo, String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("Error reading {}: {}", path, e))?;
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", meta.permissions().mode())
    };
    #[cfg(not(unix))]
    let permissions = if meta.permissions().readonly() { "readonly".into() } else { "read/write".into() };
    Ok(FileInfo {
        name: p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.to_string_lossy().to_string()),
        path,
        is_dir: meta.is_dir(),
        size: meta.len(),
        modified: modified_secs(&meta),
        created: meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0),
        permissions,
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(unix)]
fn should_try_execute(path: &Path) -> bool {
    if !is_executable(path) {
        return false;
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let lower = ext.to_ascii_lowercase();
        const BLOCKED: &[&str] = &[
            "mp4", "mp3", "avi", "mkv", "mov", "flv", "wmv", "webm", "m4v", "mpg", "mpeg", "3gp",
            "wav", "flac", "aac", "ogg", "jpg", "jpeg", "png", "gif", "bmp", "webp", "svg",
            "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "json", "xml",
            "html", "htm", "css", "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso",
        ];
        if BLOCKED.contains(&lower.as_str()) {
            return false;
        }
    }
    true
}

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: *mut core::ffi::c_void,
        lp_operation: *const u16,
        lp_file: *const u16,
        lp_parameters: *const u16,
        lp_directory: *const u16,
        n_show_cmd: i32,
    ) -> isize;
}

#[cfg(target_os = "windows")]
fn shell_execute(path: &Path, verb: Option<&str>) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    const SW_SHOWNORMAL: i32 = 1;

    let verb_wide: Option<Vec<u16>> = verb
        .map(|v| v.encode_utf16().chain(std::iter::once(0)).collect());
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: предават се валидни NUL-терминирани wide низове; резултатът > 32 означава успех.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb_wide.as_ref().map(|v| v.as_ptr()).unwrap_or(ptr::null()),
            path_wide.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result > 32 {
        Ok(())
    } else {
        Err(format!(
            "Error opening {}: ShellExecute code {}",
            path.display(),
            result
        ))
    }
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Does not exist: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        shell_execute(&p, None)?;
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(unix)]
        {
            if p.is_file() && should_try_execute(&p) {
                if std::process::Command::new(&path).spawn().is_ok() {
                    return Ok(());
                }
                // fallback към отваряне с асоциирана програма (напр. NFS с noexec)
            }
        }
        #[cfg(not(unix))]
        {
            if p.is_file() && is_executable(&p) {
                if std::process::Command::new(&path).spawn().is_ok() {
                    return Ok(());
                }
            }
        }

        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(&path).spawn();

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let result = std::process::Command::new("xdg-open").arg(&path).spawn();

        return result
            .map(|_| ())
            .map_err(|e| format!("Error opening {}: {}", path, e));
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn default_editor_command(path: &str) -> std::process::Command {
    if let Ok(editor) = std::env::var("EDITOR") {
        let mut parts = editor.split_whitespace();
        if let Some(prog) = parts.next() {
            let mut cmd = std::process::Command::new(prog);
            cmd.args(parts);
            cmd.arg(path);
            return cmd;
        }
    }
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(path);
    cmd
}

#[tauri::command]
fn edit_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Does not exist: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        match shell_execute(&p, Some("edit")) {
            Ok(()) => return Ok(()),
            Err(_) => shell_execute(&p, None)?,
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg("-e").arg(&path).spawn();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let result = default_editor_command(&path).spawn();

    #[cfg(not(target_os = "windows"))]
    {
        result
            .map(|_| ())
            .map_err(|e| format!("Error editing {}: {}", path, e))
    }
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn get_appearance() -> Result<AppSettings, String> {
    Ok(load_settings())
}

#[tauri::command]
fn set_theme(theme: String, app: tauri::AppHandle) -> Result<AppSettings, String> {
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
fn set_font(font: String, app: tauri::AppHandle) -> Result<AppSettings, String> {
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
fn set_font_size(font_size: u32, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.font_size = font_size.clamp(10, 28);
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
fn set_diff_command(command: String, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.diff_command = command;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
fn set_diff_in_terminal(in_terminal: bool, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.diff_in_terminal = in_terminal;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
fn get_favorites() -> Result<Vec<String>, String> {
    Ok(load_settings().favorites)
}

#[tauri::command]
fn set_favorites(favorites: Vec<String>, app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let mut settings = load_settings();
    settings.favorites = favorites;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings.favorites)
}

#[tauri::command]
fn get_fav_apps() -> Result<Vec<String>, String> {
    Ok(load_settings().fav_apps)
}

#[tauri::command]
fn set_fav_apps(fav_apps: Vec<String>, app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let mut settings = load_settings();
    settings.fav_apps = fav_apps;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings.fav_apps)
}

fn quote_shell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn run_in_terminal(cmd: std::process::Command, cwd: Option<&Path>) -> Result<(), String> {
    let mut line = String::new();
    if let Some(dir) = cwd {
        line.push_str("cd ");
        line.push_str(&quote_shell(&dir.to_string_lossy()));
        line.push_str(" && ");
    }
    line.push_str(&quote_shell(cmd.get_program().to_str().unwrap_or("")));
    for a in cmd.get_args() {
        line.push(' ');
        line.push_str(&quote_shell(&a.to_string_lossy()));
    }
    line.push_str("; read -p 'Press Enter to close'");
    let script = format!(
        "tell application \"Terminal\" to activate\ntell application \"Terminal\" to do script \"{}\"",
        line.replace('\\', "\\\\").replace('"', "\\\"")
    );
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .spawn()
        .map_err(|e| format!("Error opening terminal: {}", e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn quote_windows(s: &str) -> String {
    if s.is_empty() || s.contains(' ') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

#[cfg(target_os = "windows")]
fn run_in_terminal(cmd: std::process::Command, cwd: Option<&Path>) -> Result<(), String> {
    let mut cmdline = quote_windows(cmd.get_program().to_str().unwrap_or(""));
    for a in cmd.get_args() {
        cmdline.push(' ');
        cmdline.push_str(&quote_windows(&a.to_string_lossy()));
    }
    let mut wt = std::process::Command::new("wt");
    if let Some(dir) = cwd {
        wt.arg("-d").arg(dir);
    }
    wt.arg("new-tab").arg(&cmdline);
    if wt.spawn().is_ok() {
        return Ok(());
    }
    let mut start = std::process::Command::new("cmd");
    start.arg("/C").arg("start").arg("\"\"");
    if let Some(dir) = cwd {
        start.arg("/D").arg(dir);
    }
    start.arg(&cmdline);
    start
        .spawn()
        .map_err(|e| format!("Error opening terminal: {}", e))?;
    Ok(())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn run_in_terminal(cmd: std::process::Command, cwd: Option<&Path>) -> Result<(), String> {
    let mut line = String::new();
    if let Some(dir) = cwd {
        line.push_str("cd ");
        line.push_str(&quote_shell(&dir.to_string_lossy()));
        line.push_str(" && ");
    }
    line.push_str(&quote_shell(cmd.get_program().to_str().unwrap_or("")));
    for a in cmd.get_args() {
        line.push(' ');
        line.push_str(&quote_shell(&a.to_string_lossy()));
    }
    line.push_str("; read -p 'Press Enter to close'");
    let candidates: [(&str, &[&str]); 5] = [
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-e"]),
        ("xterm", &["-e"]),
    ];
    for (bin, prefix) in candidates {
        let found = std::process::Command::new(bin)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if found {
            let mut c = std::process::Command::new(bin);
            c.args(prefix);
            c.arg("sh").arg("-c").arg(&line);
            c.spawn()
                .map_err(|e| format!("Error opening terminal {}: {}", bin, e))?;
            return Ok(());
        }
    }
    Err("No terminal emulator found.".to_string())
}

fn split_command(template: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut has_token = false;
    for c in template.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    current.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                    has_token = true;
                } else if c.is_whitespace() {
                    if has_token {
                        tokens.push(std::mem::take(&mut current));
                        has_token = false;
                    }
                } else {
                    current.push(c);
                    has_token = true;
                }
            }
        }
    }
    if has_token {
        tokens.push(current);
    }
    tokens
}

fn resolve_program(tokens: &[String]) -> (String, usize) {
    let mut joined = String::new();
    for (i, t) in tokens.iter().enumerate() {
        if i > 0 {
            joined.push(' ');
        }
        joined.push_str(t);
        if Path::new(&joined).exists() {
            return (joined, i + 1);
        }
    }
    (tokens[0].clone(), 1)
}

fn build_command(template: &str, paths: &[String]) -> Result<std::process::Command, String> {
    let tokens = split_command(template);
    if tokens.is_empty() {
        return Err("Invalid command.".to_string());
    }
    let (prog, used) = resolve_program(&tokens);
    let placeholders: Vec<String> = (1..=paths.len()).map(|i| format!("%{}", i)).collect();
    let mut args: Vec<String> = Vec::new();
    let mut has_placeholder = false;
    for mut t in tokens.iter().skip(used).cloned() {
        for (i, ph) in placeholders.iter().enumerate() {
            if t.contains(ph.as_str()) {
                t = t.replace(ph.as_str(), &paths[i]);
                has_placeholder = true;
            }
        }
        args.push(t);
    }
    if !has_placeholder {
        args.extend(paths.iter().cloned());
    }
    let mut cmd = std::process::Command::new(prog);
    cmd.args(&args);
    Ok(cmd)
}

#[cfg(target_os = "macos")]
fn app_binary(app_path: &str) -> Option<String> {
    let macos_dir = std::path::Path::new(app_path).join("Contents/MacOS");
    let entries = std::fs::read_dir(macos_dir).ok()?;
    entries
        .flatten()
        .next()
        .map(|e| e.path().to_string_lossy().to_string())
}

fn launch(mut cmd: std::process::Command, in_terminal: bool, cwd: Option<&Path>) -> Result<(), String> {
    if in_terminal {
        return run_in_terminal(cmd, cwd);
    }
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    #[cfg(target_os = "macos")]
    {
        let prog = cmd.get_program().to_string_lossy().to_string();
        if prog.ends_with(".app") {
            let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
            if let Some(bin) = app_binary(&prog) {
                if std::process::Command::new(&bin).args(&args).spawn().is_ok() {
                    return Ok(());
                }
            }
            std::process::Command::new("open")
                .arg("-n")
                .arg(&prog)
                .arg("--args")
                .args(&args)
                .spawn()
                .map_err(|e| format!("Error launching {}: {}", prog, e))?;
            return Ok(());
        }
    }
    let prog = cmd.get_program().to_string_lossy().to_string();
    cmd.spawn()
        .map_err(|e| format!("Error launching {}: {}", prog, e))?;
    Ok(())
}

#[tauri::command]
fn run_diff(path_a: String, path_b: String) -> Result<(), String> {
    let settings = load_settings();
    let cmd = settings.diff_command.trim().to_string();
    if cmd.is_empty() {
        return Err("No external diff program configured.".to_string());
    }
    let command = build_command(&cmd, &[path_a, path_b])?;
    launch(command, settings.diff_in_terminal, None)
}

#[tauri::command]
fn run_edit(path: String) -> Result<(), String> {
    let settings = load_settings();
    let cmd = settings.edit_command.trim().to_string();
    if cmd.is_empty() {
        return Err("No external editor configured.".to_string());
    }
    let command = build_command(&cmd, &[path])?;
    launch(command, settings.edit_in_terminal, None)
}

#[tauri::command]
fn run_command(command: String, in_terminal: bool, cwd: String) -> Result<(), String> {
    let command = build_command(&command, &[])?;
    let dir = PathBuf::from(cwd);
    launch(command, in_terminal, Some(&dir))
}

#[tauri::command]
fn set_edit_command(command: String, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.edit_command = command;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
fn set_edit_in_terminal(in_terminal: bool, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.edit_in_terminal = in_terminal;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
fn set_show_hidden(show_hidden: bool, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.show_hidden = show_hidden;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
fn set_fuzzy_search(fuzzy_search: bool, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.fuzzy_search = fuzzy_search;
    save_settings(&settings)?;
    let _ = app.emit("appearance-changed", ());
    Ok(settings)
}

#[tauri::command]
fn set_column_widths(column_widths: ColumnWidths, _app: tauri::AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings();
    settings.column_widths = column_widths;
    save_settings(&settings)?;
    Ok(settings)
}

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
            let settings = load_settings();
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
            if window.label() != "main" {
                return;
            }
            let state = window.app_handle().state::<AppState>();
            let mut geo = state.geometry.lock().unwrap();
            match event {
                tauri::WindowEvent::Resized(size) => {
                    geo.size = Some((size.width, size.height));
                }
                tauri::WindowEvent::Moved(position) => {
                    geo.position = Some((position.x, position.y));
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_dir,
            home_dir,
            make_dir,
            rename_path,
            search_files,
            delete_path,
            copy_path,
            move_path,
            link_path,
            copy_path_progress,
            move_path_progress,
            cancel_copy,
            get_copy_progress,
            get_app_version,
            read_text_file,
            read_file_chunk,
            path_info,
            open_path,
            edit_path,
            run_diff,
            run_edit,
            run_command,
            quit_app,
            get_appearance,
            set_theme,
            set_font,
            set_font_size,
            set_diff_command,
            set_diff_in_terminal,
            set_edit_command,
            set_edit_in_terminal,
            set_show_hidden,
            set_fuzzy_search,
            set_column_widths,
            get_favorites,
            set_favorites,
            get_fav_apps,
            set_fav_apps
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app: &tauri::AppHandle, event: tauri::RunEvent| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(_window) = app.get_webview_window("main") {
                    let state = app.state::<AppState>();
                    let geo = state.geometry.lock().unwrap();
                    let mut settings = load_settings();
                    if let Some((w, h)) = geo.size {
                        settings.width = Some(w);
                        settings.height = Some(h);
                    }
                    if let Some((x, y)) = geo.position {
                        settings.x = Some(x);
                        settings.y = Some(y);
                    }
                    let _ = save_settings(&settings);
                }
            }
        });
}
