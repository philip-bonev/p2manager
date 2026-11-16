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

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::settings::load_settings;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileEntry {
    name: String,
    is_dir: bool,
    size: u64,
    modified: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirListing {
    path: String,
    parent: Option<String>,
    items: Vec<FileEntry>,
}

pub(crate) fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn entry_from_path(path: &Path) -> Result<FileEntry, String> {
    let meta =
        fs::metadata(path).map_err(|e| format!("Error reading {}: {}", path.display(), e))?;
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

#[tauri::command]
pub(crate) fn list_dir(path: String) -> Result<DirListing, String> {
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
    let parent = dir.parent().map(|p| p.to_string_lossy().to_string());
    Ok(DirListing {
        path: dir.to_string_lossy().to_string(),
        parent,
        items,
    })
}

#[tauri::command]
pub(crate) fn make_dir(parent: String, name: String) -> Result<(), String> {
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
    fs::create_dir(&path).map_err(|e| format!("Error creating folder \"{}\": {}", name, e))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn rename_path(path: String, new_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| format!("Error renaming: {}", e))?
}

#[tauri::command]
pub(crate) fn delete_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("Error reading {}: {}", path, e))?;
    let result = if meta.is_dir() {
        fs::remove_dir_all(&p)
    } else {
        fs::remove_file(&p)
    };
    result.map_err(|e| format!("Error deleting {}: {}", path, e))
}
