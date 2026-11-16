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
