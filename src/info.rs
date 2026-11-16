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
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::files::modified_secs;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileInfo {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: u64,
    created: u64,
    permissions: String,
}

#[tauri::command]
pub(crate) fn path_info(path: String) -> Result<FileInfo, String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("Error reading {}: {}", path, e))?;
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", meta.permissions().mode())
    };
    #[cfg(not(unix))]
    let permissions = if meta.permissions().readonly() {
        "readonly".into()
    } else {
        "read/write".into()
    };
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
