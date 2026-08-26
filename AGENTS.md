# Project Context & Rules: Panel Manager

## 1. System Prompt & Role
- **Role:** Expert software engineer specializing in Rust, Tauri, and frontend development. Provide clean, secure, idiomatic code.
- **Communication Language:** Always respond in Bulgarian unless asked otherwise; explain thought process and summarize code changes.
- **Code Language:** Rust backend in `/src`, vanilla HTML/CSS/JS frontend in `/web` (no framework, no build step).

## 2. Tech Stack & Architecture
- **Framework:** Tauri v2 (Rust backend + WebView frontend). Rust edition 2024.
- **Backend:** `/src/lib.rs` (single file, ~1600 lines). All Tauri commands live here.
- **Frontend files in `/web`:**
  - `index.html`, `main.js` — main window (dual-panel file manager)
  - `settings.html`, `settings.js` — runtime settings window (theme/font/font-size)
  - `i18n.js` — bg/en translations + helpers
  - `styles.css` — themes + layout
- Served as `frontendDist: "web"`. There is NO npm/package.json — do not run `npm install`; edit files directly.
- **Config:** `tauri.conf.json` (main window label `"main"`), `capabilities/default.json`, `Cargo.toml`.
- **Plugins:** opener, dialog, fs, window-state, single-instance, log.
- **Platform:** Windows, Linux, macOS. Feature differences handled with `#[cfg(...)]` in Rust.

## 3. Architecture & Data Flow (important)
- **Settings:** stored as JSON at `~/.p2manager/config.json` (cross-platform via HOME/USERPROFILE). Loaded in `.setup()` into `AppState.settings`. Window geometry (width/height/x/y) is saved on `RunEvent::ExitRequested` and restored in `.setup()`. `AppSettings` fields: `theme`, `font`, `fontSize`, `diffCommand`, `diffInTerminal`, `editCommand`, `editInTerminal`, `favorites`, `favApps`, `width`, `height`, `x`, `y`, `showHidden`, `fuzzySearch`, `columnWidths` (`{name, ext, size, date}` as `f64`).
- **Invoke arg naming:** JS passes camelCase keys (e.g. `newSettings`), Rust receives snake_case (`new_settings`) — Tauri maps automatically.
- **Windows:** main window is created from `tauri.conf.json`; settings window label `"settings"` is opened at runtime via `open_settings`.
- **Tauri v2 API notes:** `.run(closure)` is on `App` (use `.build(context).expect(...).run(...)`); `RunEvent::ExitRequested{code, api}`; `WindowEvent::Resized(PhysicalSize<u32>)` / `Moved(PhysicalPosition<i32>)`.

## 4. Backend Commands (in `src/lib.rs`)
`list_dir`, `home_dir`, `make_dir`, `rename_path`, `delete_path`, `copy_path`, `move_path`, `link_path` (hard/soft link), `read_text_file`, `path_info`, `open_path`, `edit_path` (default editor: macOS `open -e`, Linux `$EDITOR`/`xdg-open`, Windows `ShellExecuteW` verb `"edit"`), `quit_app`, `get_appearance`, `set_theme`, `set_font`, `set_font_size`, `set_diff_command`, `set_diff_in_terminal`, `set_edit_command`, `set_edit_in_terminal`, `set_show_hidden`, `set_fuzzy_search`, `set_column_widths`, `get_favorites`, `set_favorites`, `get_fav_apps`, `set_fav_apps`, `open_settings`, `search_files` (async, glob/regex, exclusions, recursive, content search), `copy_path_progress` / `move_path_progress` (async with per-file + overall progress), `get_copy_progress`, `cancel_copy`, `run_diff`, `run_edit`, `run_command`, `get_app_version`.

## 5. Frontend Key Bindings (F-key bar)
F1 help, F2 diff (compare files), F3 view file, F4 edit, F5 copy, F6 move, F7 new folder, F8 delete, F9 quick menu, F10 quit, F11 rename, F12 file info. The F5 copy dialog shows hardlink/softlink checkboxes and a progress dialog with two bars (current file + overall).

## 6. Project Commands
- **Run Dev Mode:** `cargo tauri dev`
- **Build Production:** `cargo tauri build` (bundle appears under `target/release/bundle/`)
- **Lint Rust:** `cargo clippy`
- **Format Rust:** `cargo fmt`
- **Verify JS:** `node --check web/<file>.js`
- **Version:** bump BOTH `Cargo.toml` and `tauri.conf.json` `version`.

## 7. Coding Conventions & Safety
- **Tauri commands:** return `Result<T, String>` with human-readable errors so the frontend can `alert()`/display them.
- **Style:** spaces only, 4 spaces = 1 tab. Match surrounding code; no decorative comments.
- **WebKit Bug (macOS):** WKWebView can fire spurious `click` events, e.g. when a drag-select ends on a different element. Use `mousedown` for dismiss/tab/single-press UI (overlay backdrops, modal buttons) and avoid relying on `click` for anything near selectable text. Checkboxes use `change`.
- **No new Rust crates or JS packages** unless explicitly requested.


## 8. Modal Focus Rules
- Tab/Shift+Tab while a modal is open must cycle ONLY through visible focusable elements inside the active modal (`.modal`), never background controls. Implemented via `getModalFocusables()` + the `keydown` handler; `preventDefault()` on Tab.
- Restore focus to the element that had focus before opening the modal (`lastFocused`) when it closes.
- All text in modals comes from `t()` / `tp()` — never hardcode strings.

## 9. i18n Rule (IMPORTANT)
- All UI text (labels, buttons, F-key bar, menu items, modal titles/messages, help text, error messages shown in the UI) MUST go through `web/i18n.js` via `t(key)`/`tp(key, {vars})` or `data-i18n` attributes.
- **When adding or changing any visible element, ALWAYS update BOTH the Bulgarian (`bg`) and English (`en`) dictionaries in `web/i18n.js`.** Never leave a key defined in only one language. Keep translations in sync.
- `LANG` is derived from `navigator.language` (prefix `bg` → Bulgarian, otherwise English). New keys must be added to both objects.

## 10. SVG Icons
- Top toolbar buttons use inline SVG icons (`<svg class="tb-icon">`) with `stroke="currentColor"` for theme adaptation.
- F-key bar buttons use inline SVG icons (`<svg class="fk-icon">`).
- `applyStatic()` in `i18n.js` uses `TreeWalker` to replace only text nodes, preserving SVG children.

## 11. Scoping
- Frontend logic/UI in `/web`, backend rust logic in `/src`. Do not mix concerns.
