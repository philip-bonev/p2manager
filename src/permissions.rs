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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PermissionSnapshot {
    supported: bool,
    platform: &'static str,
    mode: u32,
    attributes: u32,
}

#[tauri::command]
pub(crate) async fn get_file_permissions(path: String) -> Result<PermissionSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&path)
                .map_err(|e| format!("Error reading permissions for {}: {}", path, e))?;
            Ok(PermissionSnapshot {
                supported: true,
                platform: "unix",
                mode: metadata.permissions().mode() & 0o777,
                attributes: 0,
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            let metadata = fs::metadata(&path)
                .map_err(|e| format!("Error reading attributes for {}: {}", path, e))?;
            Ok(PermissionSnapshot {
                supported: true,
                platform: "windows",
                mode: 0,
                attributes: metadata.file_attributes() & WINDOWS_EDITABLE_ATTRIBUTES,
            })
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let _ = path;
            Ok(PermissionSnapshot {
                supported: false,
                platform: "unsupported",
                mode: 0,
                attributes: 0,
            })
        }
    })
    .await
    .map_err(|e| format!("Error reading permissions: {}", e))?
}

#[tauri::command]
pub(crate) async fn set_file_permissions(
    path: String,
    mode: u32,
    attributes: u32,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = attributes;
            if mode > 0o777 {
                return Err("Invalid permissions mode.".to_string());
            }
            let metadata = fs::metadata(&path)
                .map_err(|e| format!("Error reading permissions for {}: {}", path, e))?;
            let current = metadata.permissions().mode();
            let updated = (current & !0o777) | mode;
            fs::set_permissions(&path, fs::Permissions::from_mode(updated))
                .map_err(|e| format!("Error setting permissions for {}: {}", path, e))
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
            let _ = mode;
            if attributes & !WINDOWS_EDITABLE_ATTRIBUTES != 0 {
                return Err("Invalid Windows file attributes.".to_string());
            }
            let current = fs::metadata(&path)
                .map_err(|e| format!("Error reading attributes for {}: {}", path, e))?
                .file_attributes();
            let mut updated =
                (current & !WINDOWS_EDITABLE_ATTRIBUTES & !FILE_ATTRIBUTE_NORMAL) | attributes;
            if updated == 0 {
                updated = FILE_ATTRIBUTE_NORMAL;
            }
            let mut wide_path: Vec<u16> = std::ffi::OsStr::new(&path).encode_wide().collect();
            wide_path.push(0);
            let result = unsafe { set_windows_file_attributes(wide_path.as_ptr(), updated) };
            if result == 0 {
                Err(format!(
                    "Error setting attributes for {}: {}",
                    path,
                    std::io::Error::last_os_error()
                ))
            } else {
                Ok(())
            }
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let _ = (path, mode, attributes);
            Err("File permissions are not supported on this platform.".to_string())
        }
    })
    .await
    .map_err(|e| format!("Error setting permissions: {}", e))?
}

#[cfg(windows)]
const WINDOWS_EDITABLE_ATTRIBUTES: u32 = 0x01 | 0x02 | 0x04 | 0x20;

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    #[link_name = "SetFileAttributesW"]
    fn set_windows_file_attributes(path: *const u16, attributes: u32) -> i32;
}
