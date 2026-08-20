use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

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

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_dir,
            home_dir,
            make_dir,
            delete_path,
            copy_path,
            move_path,
            read_text_file,
            path_info,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}