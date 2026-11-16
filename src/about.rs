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

#[tauri::command]
pub(crate) fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

const LICENSE_TEXT: &str = include_str!("../LICENSE");

#[tauri::command]
pub(crate) fn get_about_info() -> Result<serde_json::Value, String> {
    let date = option_env!("BUILD_DATE").unwrap_or("");
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "date": date,
        "license": LICENSE_TEXT,
    }))
}
