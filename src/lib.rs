use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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
    width: Option<u32>,
    height: Option<u32>,
    x: Option<i32>,
    y: Option<i32>,
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
            width: None,
            height: None,
            x: None,
            y: None,
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
        fs::create_dir_all(parent).map_err(|e| format!("Грешка при запис на настройките: {}", e))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Грешка при запис на настройките: {}", e))
}

fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn entry_from_path(path: &Path) -> Result<FileEntry, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Грешка при четене на {}: {}", path.display(), e))?;
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

fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("Грешка при създаване на {}: {}", dst.display(), e))?;
        for entry in fs::read_dir(src).map_err(|e| format!("Грешка при четене на {}: {}", src.display(), e))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let child_dst = dst.join(entry.file_name());
            copy_recursive(&entry.path(), &child_dst)?;
        }
    } else {
        fs::copy(src, dst)
            .map_err(|e| format!("Грешка при копиране на {}: {}", src.display(), e))?;
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

#[tauri::command]
fn list_dir(path: String) -> Result<DirListing, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("Не е директория: {}", path));
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("Грешка при четене на {}: {}", path, e))? {
        let entry = entry.map_err(|e| e.to_string())?;
        match entry_from_path(&entry.path()) {
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
fn home_dir() -> Result<String, String> {
    home_dir_path()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Не може да се определи домашната директория.".to_string())
}

#[tauri::command]
fn make_dir(parent: String, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Името на папката не може да е празно.".to_string());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("Името съдържа невалидни символи.".to_string());
    }
    let path = PathBuf::from(&parent).join(name);
    if path.exists() {
        return Err(format!("„{}“ вече съществува.", name));
    }
    fs::create_dir(&path)
        .map_err(|e| format!("Грешка при създаване на папка „{}“: {}", name, e))?;
    Ok(())
}

#[tauri::command]
fn delete_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("Грешка при четене на {}: {}", path, e))?;
    let result = if meta.is_dir() {
        fs::remove_dir_all(&p)
    } else {
        fs::remove_file(&p)
    };
    result.map_err(|e| format!("Грешка при изтриване на {}: {}", path, e))
}

#[tauri::command]
fn copy_path(src: String, dst_dir: String) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst_dir);
    if !dst_path.is_dir() {
        return Err(format!("Не е директория: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Невалиден изходен път.".to_string())?;
    if src_path.parent().map(|p| p == dst_path.as_path()).unwrap_or(false) {
        return Err("Файлът вече е в тази папка.".to_string());
    }
    if is_descendant(&dst_path, &src_path) {
        return Err("Не може да копирате папка в самата нея.".to_string());
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
        return Err(format!("Не е директория: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Невалиден изходен път.".to_string())?;
    if src_path.parent().map(|p| p == dst_path.as_path()).unwrap_or(false) {
        return Err("Файлът вече е в тази папка.".to_string());
    }
    if is_descendant(&dst_path, &src_path) {
        return Err("Не може да преместите папка в самата нея.".to_string());
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
            del.map_err(|e| format!("Копирано, но не може да се изтрие източникът: {}", e))?;
            Ok(dest.to_string_lossy().to_string())
        }
    }
}

#[tauri::command]
fn link_path(src: String, dst_dir: String, hard: bool) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dst_path = PathBuf::from(&dst_dir);
    if !dst_path.is_dir() {
        return Err(format!("Не е директория: {}", dst_dir));
    }
    let name = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Невалиден изходен път.".to_string())?;
    let dest = resolve_dest(&dst_path, &name);

    if hard {
        fs::hard_link(&src_path, &dest).map_err(|e| {
            format!("Грешка при създаване на hardlink на {}: {}", src_path.display(), e)
        })?;
    } else {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&src_path, &dest).map_err(|e| {
                format!("Грешка при създаване на symlink на {}: {}", src_path.display(), e)
            })?;
        }
        #[cfg(windows)]
        {
            let meta = fs::metadata(&src_path).map_err(|e| {
                format!("Грешка при четене на {}: {}", src_path.display(), e)
            })?;
            let result = if meta.is_dir() {
                std::os::windows::fs::symlink_dir(&src_path, &dest)
            } else {
                std::os::windows::fs::symlink_file(&src_path, &dest)
            };
            result.map_err(|e| {
                format!("Грешка при създаване на symlink на {}: {}", src_path.display(), e)
            })?;
        }
    }
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let data = fs::read(Path::new(&path))
        .map_err(|e| format!("Не може да се прочете файлът {}: {}", path, e))?;
    let capped = if data.len() > 1_000_000 { &data[..1_000_000] } else { &data[..] };
    match std::str::from_utf8(capped) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Err("Бинарен файл — не може да се прегледа като текст.".to_string()),
    }
}

#[tauri::command]
fn path_info(path: String) -> Result<FileInfo, String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("Грешка при четене на {}: {}", path, e))?;
    let mut permissions = String::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions = format!("{:o}", meta.permissions().mode());
    }
    #[cfg(not(unix))]
    {
        permissions = if meta.permissions().readonly() { "readonly".into() } else { "read/write".into() };
    }
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
            "Грешка при отваряне на {}: ShellExecute код {}",
            path.display(),
            result
        ))
    }
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Не съществува: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        shell_execute(&p, None)?;
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        if p.is_file() && is_executable(&p) {
            std::process::Command::new(&path)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("Грешка при изпълнение на {}: {}", path, e))?;
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(&path).spawn();

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let result = std::process::Command::new("xdg-open").arg(&path).spawn();

        return result
            .map(|_| ())
            .map_err(|e| format!("Грешка при отваряне на {}: {}", path, e));
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
    std::process::Command::new("xdg-open").arg(path)
}

#[tauri::command]
fn edit_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Не съществува: {}", path));
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

    result
        .map(|_| ())
        .map_err(|e| format!("Грешка при редакция на {}: {}", path, e))
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
        return Err("Невалидна тема.".to_string());
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
        return Err("Невалиден шрифт.".to_string());
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
        .map_err(|e| format!("Грешка при отваряне на терминал: {}", e))?;
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
        .map_err(|e| format!("Грешка при отваряне на терминал: {}", e))?;
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
                .map_err(|e| format!("Грешка при отваряне на терминал {}: {}", bin, e))?;
            return Ok(());
        }
    }
    Err("Не е намерен терминален емулатор.".to_string())
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
        return Err("Невалидна команда.".to_string());
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
                std::process::Command::new(&bin)
                    .args(&args)
                    .spawn()
                    .map_err(|e| format!("Грешка при стартиране на {}: {}", prog, e))?;
                return Ok(());
            }
            std::process::Command::new("open")
                .arg("-n")
                .arg(&prog)
                .arg("--args")
                .args(&args)
                .spawn()
                .map_err(|e| format!("Грешка при стартиране на {}: {}", prog, e))?;
            return Ok(());
        }
    }
    let prog = cmd.get_program().to_string_lossy().to_string();
    cmd.spawn()
        .map_err(|e| format!("Грешка при стартиране на {}: {}", prog, e))?;
    Ok(())
}

#[tauri::command]
fn run_diff(path_a: String, path_b: String) -> Result<(), String> {
    let settings = load_settings();
    let cmd = settings.diff_command.trim().to_string();
    if cmd.is_empty() {
        return Err("Не е зададена външна програма за сравняване.".to_string());
    }
    let command = build_command(&cmd, &[path_a, path_b])?;
    launch(command, settings.diff_in_terminal, None)
}

#[tauri::command]
fn run_edit(path: String) -> Result<(), String> {
    let settings = load_settings();
    let cmd = settings.edit_command.trim().to_string();
    if cmd.is_empty() {
        return Err("Не е зададен външен редактор.".to_string());
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
fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(&app, "settings", tauri::WebviewUrl::App("settings.html".into()))
        .title("Настройки")
        .inner_size(360.0, 620.0)
        .resizable(false)
        .build()
        .map_err(|e| format!("Грешка при отваряне на настройките: {}", e))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            geometry: Mutex::new(WindowGeometry::default()),
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
            delete_path,
            copy_path,
            move_path,
            link_path,
            read_text_file,
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
            open_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app: &tauri::AppHandle, event: tauri::RunEvent| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(window) = app.get_webview_window("main") {
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