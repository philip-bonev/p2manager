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
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

use crate::state::AppState;

fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("Error creating {}: {}", dst.display(), e))?;
        for entry in
            fs::read_dir(src).map_err(|e| format!("Error reading {}: {}", src.display(), e))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let child_dst = dst.join(entry.file_name());
            copy_recursive(&entry.path(), &child_dst)?;
        }
    } else {
        fs::copy(src, dst).map_err(|e| format!("Error copying {}: {}", src.display(), e))?;
    }
    Ok(())
}

fn is_descendant(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
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
pub(crate) struct CopyProgress {
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
    let mut src_file =
        fs::File::open(src).map_err(|e| format!("Error opening {}: {}", src.display(), e))?;
    let mut dst_file =
        fs::File::create(dst).map_err(|e| format!("Error creating {}: {}", dst.display(), e))?;
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
                serde_json::to_value(CopyProgress {
                    id: id.to_string(),
                    copied,
                    total,
                    path: src.to_string_lossy().to_string(),
                })
                .unwrap_or_default(),
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
        fs::create_dir_all(dst).map_err(|e| format!("Error creating {}: {}", dst.display(), e))?;
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
        state
            .cancel_flags
            .lock()
            .unwrap()
            .insert(id.to_string(), flag.clone());
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

#[derive(Clone, serde::Serialize)]
pub(crate) struct DeleteProgress {
    id: String,
    deleted: u64,
    total: u64,
    path: String,
}

fn count_entries(path: &Path) -> u64 {
    if path.is_dir() {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                count += 1;
                if entry.path().is_dir() {
                    count += count_entries(&entry.path());
                }
            }
        }
        count
    } else {
        1
    }
}

fn report_delete_progress(app: &tauri::AppHandle, id: &str, counter: u64, total: u64, path: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(v) = serde_json::to_value(DeleteProgress {
            id: id.to_string(),
            deleted: counter,
            total,
            path: path.to_string(),
        }) {
            state.progress.lock().unwrap().insert(id.to_string(), v);
        }
    }
}

fn delete_recursive_progress(
    path: &Path,
    app: &tauri::AppHandle,
    id: &str,
    counter: &mut u64,
    total: u64,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("CANCELLED".to_string());
    }
    if path.is_dir() {
        let entries: Vec<_> = fs::read_dir(path)
            .map_err(|e| format!("Error reading {}: {}", path.display(), e))?
            .flatten()
            .collect();
        for entry in entries {
            let p = entry.path();
            if p.is_dir() {
                delete_recursive_progress(&p, app, id, counter, total, cancel)?;
            } else {
                if cancel.load(Ordering::Relaxed) {
                    return Err("CANCELLED".to_string());
                }
                fs::remove_file(&p)
                    .map_err(|e| format!("Error deleting {}: {}", p.display(), e))?;
                *counter += 1;
                report_delete_progress(app, id, *counter, total, &p.to_string_lossy());
            }
        }
        fs::remove_dir(path).map_err(|e| format!("Error deleting {}: {}", path.display(), e))?;
        *counter += 1;
        report_delete_progress(app, id, *counter, total, &path.to_string_lossy());
    } else {
        if cancel.load(Ordering::Relaxed) {
            return Err("CANCELLED".to_string());
        }
        fs::remove_file(path).map_err(|e| format!("Error deleting {}: {}", path.display(), e))?;
        *counter += 1;
        report_delete_progress(app, id, *counter, total, &path.to_string_lossy());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn delete_path_progress(
    app: tauri::AppHandle,
    paths: Vec<String>,
    id: String,
) -> Result<(), String> {
    let total: u64 = paths.iter().map(|p| count_entries(&PathBuf::from(p))).sum();
    let cancel = register_cancel(&app, &id);
    let app2 = app.clone();
    let id2 = id.clone();
    let paths2 = paths.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut counter = 0u64;
        for p in &paths2 {
            let path = PathBuf::from(p);
            delete_recursive_progress(&path, &app2, &id2, &mut counter, total, &cancel)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Error deleting: {}", e))?;
    unregister_cancel(&app, &id);
    unregister_progress(&app, &id);
    result
}

#[tauri::command]
pub(crate) async fn copy_path(src: String, dst_dir: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let src_path = PathBuf::from(&src);
        let dst_path = PathBuf::from(&dst_dir);
        if !dst_path.is_dir() {
            return Err(format!("Not a directory: {}", dst_dir));
        }
        let name = src_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| "Invalid source path.".to_string())?;
        if src_path
            .parent()
            .map(|p| p == dst_path.as_path())
            .unwrap_or(false)
        {
            return Err("File is already in this folder.".to_string());
        }
        if is_descendant(&dst_path, &src_path) {
            return Err("Cannot copy a folder into itself.".to_string());
        }
        let dest = resolve_dest(&dst_path, &name);
        copy_recursive(&src_path, &dest)?;
        Ok(dest.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Error copying: {}", e))?
}

#[tauri::command]
pub(crate) async fn move_path(src: String, dst_dir: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let src_path = PathBuf::from(&src);
        let dst_path = PathBuf::from(&dst_dir);
        if !dst_path.is_dir() {
            return Err(format!("Not a directory: {}", dst_dir));
        }
        let name = src_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| "Invalid source path.".to_string())?;
        if src_path
            .parent()
            .map(|p| p == dst_path.as_path())
            .unwrap_or(false)
        {
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
    })
    .await
    .map_err(|e| format!("Error moving: {}", e))?
}

#[tauri::command]
pub(crate) async fn link_path(src: String, dst_dir: String, hard: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
            fs::hard_link(&src_path, &dest)
                .map_err(|e| format!("Error creating hardlink of {}: {}", src_path.display(), e))?;
        } else {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&src_path, &dest).map_err(|e| {
                    format!("Error creating symlink of {}: {}", src_path.display(), e)
                })?;
            }
            #[cfg(windows)]
            {
                let meta = fs::metadata(&src_path)
                    .map_err(|e| format!("Error reading {}: {}", src_path.display(), e))?;
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
    })
    .await
    .map_err(|e| format!("Error creating link: {}", e))?
}

#[tauri::command]
pub(crate) async fn copy_path_progress(
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
    if src_path
        .parent()
        .map(|p| p == dst_path.as_path())
        .unwrap_or(false)
    {
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
pub(crate) async fn move_path_progress(
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
    if src_path
        .parent()
        .map(|p| p == dst_path.as_path())
        .unwrap_or(false)
    {
        return Err("File is already in this folder.".to_string());
    }
    if is_descendant(&dst_path, &src_path) {
        return Err("Cannot move a folder into itself.".to_string());
    }
    let dest = resolve_dest(&dst_path, &name);
    let cancel = register_cancel(&app, &id);
    let app2 = app.clone();
    let id2 = id.clone();
    let src2 = src_path.clone();
    let dest2 = dest.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        if fs::rename(&src2, &dest2).is_err() {
            copy_recursive_progress(&src2, &dest2, &app2, &id2, &cancel)?;
            let meta = fs::metadata(&src2).map_err(|e| e.to_string())?;
            let del = if meta.is_dir() {
                fs::remove_dir_all(&src2)
            } else {
                fs::remove_file(&src2)
            };
            del.map_err(|e| format!("Copied, but source could not be deleted: {}", e))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Error moving: {}", e))?;
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
pub(crate) fn cancel_copy(id: String, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Some(flag) = state.cancel_flags.lock().unwrap().get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn get_copy_progress(
    id: String,
    app: tauri::AppHandle,
) -> Result<Option<serde_json::Value>, String> {
    Ok(app
        .try_state::<AppState>()
        .and_then(|s| s.progress.lock().unwrap().get(&id).cloned()))
}
