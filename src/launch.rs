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

use std::fs;
use std::path::{Path, PathBuf};

use crate::settings::load_settings;

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
            "wav", "flac", "aac", "ogg", "jpg", "jpeg", "png", "gif", "bmp", "webp", "svg", "pdf",
            "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "json", "xml", "html", "htm",
            "css", "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso",
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

    let verb_wide: Option<Vec<u16>> =
        verb.map(|v| v.encode_utf16().chain(std::iter::once(0)).collect());
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: предават се валидни NUL-терминирани wide низове; резултатът > 32 означава успех.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb_wide
                .as_ref()
                .map(|v| v.as_ptr())
                .unwrap_or(ptr::null()),
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
pub(crate) fn open_path(path: String) -> Result<(), String> {
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
        let result = {
            let mut cmd = std::process::Command::new("open");
            // macOS 27 does not deliver the document to an already running
            // application, so files are always opened in a new instance.
            if !p.is_dir() {
                cmd.arg("-n");
            }
            cmd.arg(&path).spawn()
        };

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
pub(crate) fn edit_path(path: String) -> Result<(), String> {
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
    let result = std::process::Command::new("open")
        .arg("-e")
        .arg(&path)
        .spawn();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let result = default_editor_command(&path).spawn();

    #[cfg(not(target_os = "windows"))]
    {
        result
            .map(|_| ())
            .map_err(|e| format!("Error editing {}: {}", path, e))
    }
}

#[cfg(unix)]
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

fn launch(
    mut cmd: std::process::Command,
    in_terminal: bool,
    cwd: Option<&Path>,
) -> Result<(), String> {
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
            let args: Vec<String> = cmd
                .get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect();
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
pub(crate) fn run_diff(path_a: String, path_b: String) -> Result<(), String> {
    let settings = load_settings();
    let cmd = settings.diff_command.trim().to_string();
    if cmd.is_empty() {
        return Err("No external diff program configured.".to_string());
    }
    let command = build_command(&cmd, &[path_a, path_b])?;
    launch(command, settings.diff_in_terminal, None)
}

#[tauri::command]
pub(crate) fn run_edit(path: String) -> Result<(), String> {
    let settings = load_settings();
    let cmd = settings.edit_command.trim().to_string();
    if cmd.is_empty() {
        return Err("No external editor configured.".to_string());
    }
    let command = build_command(&cmd, &[path])?;
    launch(command, settings.edit_in_terminal, None)
}

#[tauri::command]
pub(crate) fn run_command(command: String, in_terminal: bool, cwd: String) -> Result<(), String> {
    let command = build_command(&command, &[])?;
    let dir = PathBuf::from(cwd);
    launch(command, in_terminal, Some(&dir))
}
