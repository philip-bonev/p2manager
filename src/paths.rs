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

use std::path::PathBuf;

pub(crate) fn home_dir_path() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var(key).ok().map(PathBuf::from)
}

#[tauri::command]
pub(crate) fn home_dir() -> Result<String, String> {
    home_dir_path()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Cannot determine home directory.".to_string())
}

#[tauri::command]
pub(crate) fn expand_env(path: String) -> Result<String, String> {
    let mut result = String::new();
    let chars: Vec<char> = path.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if cfg!(windows) && chars[i] == '%' {
            i += 1;
            let start = i;
            while i < len && chars[i] != '%' {
                i += 1;
            }
            if i < len {
                let var_name: String = chars[start..i].iter().collect();
                i += 1;
                match std::env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => {
                        result.push('%');
                        result.push_str(&var_name);
                        result.push('%');
                    }
                }
            } else {
                result.push('%');
                result.push_str(&chars[start..].iter().collect::<String>());
            }
        } else if !cfg!(windows) && chars[i] == '$' {
            i += 1;
            if i < len && chars[i] == '{' {
                i += 1;
                let start = i;
                while i < len && chars[i] != '}' {
                    i += 1;
                }
                let var_name: String = chars[start..i].iter().collect();
                if i < len {
                    i += 1;
                }
                match std::env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => {
                        result.push_str("${");
                        result.push_str(&var_name);
                        result.push('}');
                    }
                }
            } else if i < len && chars[i].is_ascii_alphanumeric() || i < len && chars[i] == '_' {
                let start = i;
                while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let var_name: String = chars[start..i].iter().collect();
                match std::env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => {
                        result.push('$');
                        result.push_str(&var_name);
                    }
                }
            } else {
                result.push('$');
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    Ok(result)
}
