use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{IncludeBackground, styled_line_to_highlighted_html};
use syntect::parsing::SyntaxSet;
use tauri::Manager;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static SYNTAX_THEMES: OnceLock<ThemeSet> = OnceLock::new();

#[tauri::command]
pub(crate) fn read_text_file(path: String) -> Result<String, String> {
    let data =
        fs::read(Path::new(&path)).map_err(|e| format!("Cannot read file {}: {}", path, e))?;
    let capped = if data.len() > 1_000_000 {
        &data[..1_000_000]
    } else {
        &data[..]
    };
    Ok(String::from_utf8_lossy(capped).to_string())
}

#[tauri::command]
pub(crate) fn read_file_chunk(path: String, offset: u64, limit: u64) -> Result<String, String> {
    let meta =
        fs::metadata(Path::new(&path)).map_err(|e| format!("Cannot read file {}: {}", path, e))?;
    let file_size = meta.len();
    if offset >= file_size {
        return Ok(String::new());
    }
    let end = std::cmp::min(offset + limit, file_size);
    let len = (end - offset) as usize;
    let mut buf = vec![0u8; len];
    let mut f = fs::File::open(Path::new(&path))
        .map_err(|e| format!("Cannot open file {}: {}", path, e))?;
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Error seeking: {}", e))?;
    f.read_exact(&mut buf)
        .map_err(|e| format!("Error reading: {}", e))?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

#[tauri::command]
pub(crate) async fn authorize_view_image(
    path: String,
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<String, String> {
    if window.label() != "view" {
        return Err("Images can only be opened in the file viewer.".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let source = PathBuf::from(&path);
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp"
        ) {
            return Err("Unsupported image format.".to_string());
        }
        let canonical =
            fs::canonicalize(&source).map_err(|e| format!("Cannot open image {}: {}", path, e))?;
        if !fs::metadata(&canonical)
            .map_err(|e| format!("Cannot read image {}: {}", path, e))?
            .is_file()
        {
            return Err("The selected image is not a file.".to_string());
        }
        app.asset_protocol_scope()
            .allow_file(&canonical)
            .map_err(|e| format!("Cannot authorize image {}: {}", path, e))?;
        Ok(canonical.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Error opening image: {}", e))?
}

fn highlight_syntax_text(
    text: &str,
    extension: &str,
    dark: bool,
) -> Result<Option<Vec<String>>, String> {
    let syntaxes = SYNTAXES.get_or_init(two_face::syntax::extra_newlines);
    let themes = SYNTAX_THEMES.get_or_init(ThemeSet::load_defaults);
    let extension = extension.trim_start_matches('.');
    let syntax = syntaxes.find_syntax_by_extension(extension).or_else(|| {
        text.lines()
            .next()
            .and_then(|line| syntaxes.find_syntax_by_first_line(line))
    });
    let Some(syntax) = syntax else {
        return Ok(None);
    };
    if syntax.name == "Plain Text" {
        return Ok(None);
    }
    let theme_name = if dark {
        "base16-ocean.dark"
    } else {
        "InspiredGitHub"
    };
    let theme = themes
        .themes
        .get(theme_name)
        .ok_or_else(|| format!("Syntax theme {} is unavailable.", theme_name))?;
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut result = Vec::new();
    for line in text.split_inclusive('\n') {
        let line_with_newline;
        let line = if line.ends_with('\n') {
            line
        } else {
            line_with_newline = format!("{}\n", line);
            &line_with_newline
        };
        let regions = highlighter
            .highlight_line(line, syntaxes)
            .map_err(|e| format!("Syntax highlighting failed: {}", e))?;
        let html_regions: Vec<_> = regions
            .into_iter()
            .filter_map(|(style, token)| {
                let token = token.strip_suffix('\n').unwrap_or(token);
                let token = token.strip_suffix('\r').unwrap_or(token);
                (!token.is_empty()).then_some((style, token))
            })
            .collect();
        let html = styled_line_to_highlighted_html(&html_regions, IncludeBackground::No)
            .map_err(|e| format!("Could not render highlighted code: {}", e))?;
        result.push(html);
    }
    if text.is_empty() || text.ends_with('\n') {
        result.push(String::new());
    }
    Ok(Some(result))
}

#[tauri::command]
pub(crate) async fn highlight_code_text(
    text: String,
    extension: String,
    dark: bool,
    window: tauri::WebviewWindow,
) -> Result<Option<Vec<String>>, String> {
    if window.label() != "view" {
        return Err("Syntax highlighting is only available in the file viewer.".to_string());
    }
    if text.len() > 2 * 1024 * 1024 {
        return Err("Syntax highlighting is limited to 2 MiB.".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || highlight_syntax_text(&text, &extension, dark))
        .await
        .map_err(|e| format!("Syntax highlighting failed: {}", e))?
}

#[cfg(test)]
mod syntax_highlighting_tests {
    use super::highlight_syntax_text;

    #[test]
    fn bundled_syntaxes_cover_requested_file_formats() {
        let syntaxes = two_face::syntax::extra_newlines();
        for extension in ["js", "html", "css", "json", "yaml", "md", "rs", "toml"] {
            assert!(
                syntaxes.find_syntax_by_extension(extension).is_some(),
                "missing Syntect syntax for .{extension}"
            );
        }
    }

    #[test]
    fn highlighted_lines_do_not_include_line_breaks() {
        let highlighted = highlight_syntax_text("[section]\nkey = 1\n", "toml", true)
            .unwrap()
            .unwrap();
        assert_eq!(highlighted.len(), 3);
        assert!(
            highlighted
                .iter()
                .all(|line| !line.contains('\n') && !line.contains('\r'))
        );
    }
}
