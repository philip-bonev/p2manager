use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchParams {
    base_path: String,
    exclusions: String,
    pattern: String,
    pattern_mode: String,
    ignore_case: bool,
    recursive: bool,
    content_enabled: bool,
    content_pattern: String,
    content_mode: String,
    content_ignore_case: bool,
}

fn glob_to_regex(pattern: &str) -> String {
    let mut re = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
    }
    re.push('$');
    re
}

fn build_file_matcher(
    pattern: &str,
    mode: &str,
    ignore_case: bool,
) -> Result<Option<regex::Regex>, String> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return Ok(None);
    }
    let re_str = if mode == "regexp" {
        pat.to_string()
    } else {
        glob_to_regex(pat)
    };
    let mut builder = regex::RegexBuilder::new(&re_str);
    builder.case_insensitive(ignore_case);
    builder
        .build()
        .map(Some)
        .map_err(|e| format!("Invalid pattern: {}", e))
}

fn build_content_matcher(
    pattern: &str,
    mode: &str,
    ignore_case: bool,
) -> Result<(Option<regex::Regex>, Option<String>), String> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return Ok((None, None));
    }
    if mode == "regexp" {
        let mut builder = regex::RegexBuilder::new(pat);
        builder.case_insensitive(ignore_case);
        let re = builder
            .build()
            .map_err(|e| format!("Invalid content pattern: {}", e))?;
        Ok((Some(re), None))
    } else {
        let plain = if ignore_case {
            pat.to_lowercase()
        } else {
            pat.to_string()
        };
        Ok((None, Some(plain)))
    }
}

fn file_content_matches(
    path: &Path,
    content_re: &Option<regex::Regex>,
    content_plain: &Option<String>,
    content_ignore_case: bool,
) -> bool {
    if content_re.is_none() && content_plain.is_none() {
        return true;
    }
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if data.len() > 20 * 1024 * 1024 {
        return false;
    }
    let text = String::from_utf8_lossy(&data);
    if let Some(re) = content_re {
        return re.is_match(&text);
    }
    if let Some(plain) = content_plain {
        if content_ignore_case {
            return text.to_lowercase().contains(plain);
        } else {
            return text.contains(plain);
        }
    }
    true
}

fn search_walk(
    dir: &Path,
    exclusions: &[String],
    file_re: &Option<regex::Regex>,
    content_re: &Option<regex::Regex>,
    content_plain: &Option<String>,
    content_enabled: bool,
    content_ignore_case: bool,
    recursive: bool,
    results: &mut Vec<String>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let excluded = exclusions.iter().any(|e| e == name);
        if excluded {
            continue;
        }
        let name_matches = match file_re {
            Some(re) => re.is_match(name),
            None => true,
        };
        let is_dir = p.is_dir();
        if content_enabled {
            if name_matches {
                if is_dir {
                    if recursive {
                        search_walk(
                            &p,
                            exclusions,
                            file_re,
                            content_re,
                            content_plain,
                            content_enabled,
                            content_ignore_case,
                            recursive,
                            results,
                        );
                    }
                } else if file_content_matches(&p, content_re, content_plain, content_ignore_case) {
                    results.push(p.to_string_lossy().to_string());
                }
            } else if is_dir && recursive {
                search_walk(
                    &p,
                    exclusions,
                    file_re,
                    content_re,
                    content_plain,
                    content_enabled,
                    content_ignore_case,
                    recursive,
                    results,
                );
            }
        } else {
            if name_matches {
                results.push(p.to_string_lossy().to_string());
            }
            if is_dir && recursive {
                search_walk(
                    &p,
                    exclusions,
                    file_re,
                    content_re,
                    content_plain,
                    content_enabled,
                    content_ignore_case,
                    recursive,
                    results,
                );
            }
        }
    }
}

#[tauri::command]
pub(crate) async fn search_files(params: SearchParams) -> Result<Vec<String>, String> {
    let base = PathBuf::from(params.base_path.clone());
    if !base.is_dir() {
        return Err(format!("Not a directory: {}", params.base_path));
    }
    let exclusions: Vec<String> = params
        .exclusions
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let file_re = build_file_matcher(&params.pattern, &params.pattern_mode, params.ignore_case)?;
    let (content_re, content_plain) = if params.content_enabled {
        build_content_matcher(
            &params.content_pattern,
            &params.content_mode,
            params.content_ignore_case,
        )?
    } else {
        (None, None)
    };
    let recursive = params.recursive;
    let content_enabled = params.content_enabled;
    let content_ignore_case = params.content_ignore_case;
    let res = tauri::async_runtime::spawn_blocking(move || {
        let mut results = Vec::new();
        search_walk(
            &base,
            &exclusions,
            &file_re,
            &content_re,
            &content_plain,
            content_enabled,
            content_ignore_case,
            recursive,
            &mut results,
        );
        results.sort();
        results
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(res)
}
