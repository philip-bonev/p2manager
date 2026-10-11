# Project Context & Rules: Panel Manager

## 1. System Prompt & Role
- **Role:** Expert software engineer specializing in Rust, Tauri, and frontend development. Provide clean, secure, idiomatic code.
- **Communication Language:** Always respond in Bulgarian unless asked otherwise; explain thought process and summarize code changes.
- **Code Language:** Rust backend in `/src`, vanilla HTML/CSS/JS frontend in `/web` (no framework, no build step).

## 2. Tech Stack & Architecture
- **Framework:** Tauri v2 (Rust backend + WebView frontend). Rust edition 2024.
- **Backend:** `/src/lib.rs` (single file, ~1900 lines). All Tauri commands live here.
- **Frontend files in `/web`:**
  - `index.html`, `main.js`, `styles.css` — main window (dual-panel file manager)
  - `settings.html`, `settings.js`, `settings.css` — settings window (theme/font/font-size/diff/edit/show hidden)
  - `help.html`, `help.js`, `help.css` — help/about window (help text + license tabs)
  - `search.html`, `search.js`, `search.css` — file search window (glob/regex + content search)
  - `view.html`, `view.js`, `view.css` — file viewer window (F3; lazy text chunks via `read_file_chunk`, native image display for JPEG/PNG/GIF/WebP/BMP)
  - `info.html`, `info.js`, `info.css` — file info window (F12; `path_info` rendered as a table)
  - `permissions.html`, `permissions.js`, `permissions.css` — POSIX permissions window (F10; owner/group/others rwx)
  - `common.css` — shared styles: themes, base, scrollbars, form widgets (`.opt`, `.row`, `.hint`, `select`, `input[type=text]`, `hr`), window chrome (`.win-header`, `.win-title`, `.win-close`, `.win-body`)
  - `appearance.js` — shared module for theme/font application + `appearance-changed`/`onThemeChanged` listeners (`initAppearance(onChange)`)
  - `i18n.js` — bg/en translations + helpers (shared by all windows)
- Served as `frontendDist: "web"`. There is NO npm/package.json — do not run `npm install`; edit files directly.
- **Config:** `tauri.conf.json` (main window label `"main"`), `capabilities/default.json`, `Cargo.toml`.
- **Plugins:** opener, dialog, fs, window-state, single-instance, log.
- **Platform:** Windows, Linux, macOS. Feature differences handled with `#[cfg(...)]` in Rust.

## 3. Architecture & Data Flow (important)
- **Settings:** stored as JSON at `~/.p2manager/config.json` (cross-platform via HOME/USERPROFILE). Loaded in `.setup()` into `AppState.settings`. Window geometry (width/height/x/y) is saved on `RunEvent::ExitRequested` and restored in `.setup()`. `AppSettings` fields: `theme`, `font`, `fontSize`, `diffCommand`, `diffInTerminal`, `editCommand`, `editInTerminal`, `favorites`, `favApps`, `width`, `height`, `x`, `y`, `showHidden`, `fuzzySearch`, `columnWidths` (`{name, ext, size, date}` as `f64`).
- **Invoke arg naming:** JS passes camelCase keys (e.g. `newSettings`), Rust receives snake_case (`new_settings`) — Tauri maps automatically.
- **Windows:** main window is created from `tauri.conf.json`. Secondary windows (`"settings"`, `"help"`, `"search"`, `"view"`, `"info"`, `"permissions"`) are created at runtime via `open_child_window()` (`open_settings` / `open_help` / `open_search` / `open_view` / `open_info` / `open_permissions` commands): centered on the main window's center (physical coords + monitor clamp), focused if already open. Each has its own html/js/css; shared CSS in `common.css`, shared theme/font logic in `appearance.js`.
- **Close logic:** closing `main` closes all secondary windows and exits the app. Closing a secondary window closes only itself (main keeps the app alive); `RunEvent::ExitRequested` has an explicit check (`code.is_none() && main exists → api.prevent_exit()`).
- **Search window events:** `search.js` emits `search-window-ready` on load; main responds with `emit("search-open", { basePath })`. Choosing a result emits `search-navigate", { path }` and the search window closes itself; main navigates the active panel.
- **View/info window events:** main stores `pendingViewPath`/`pendingInfoPath`, invokes `open_view`/`open_info`, then emits `view-open`/`info-open` with `{ path }` (covers already-open windows). The windows emit `view-window-ready`/`info-window-ready` on load; main re-sends the pending path (covers first open, when the listener was not yet registered). Reopening on another file reuses the window and just reloads content.
- **Permissions window events:** main stores `pendingPermissionsPath`, opens/focuses `permissions`, and emits `permissions-open` with `{ path }`; the window emits `permissions-window-ready` on load so the main window can resend the path for first-open timing. Permission editing uses POSIX rwx bits on Unix and read-only/archive/system/hidden attributes on Windows.
- **Tauri v2 API notes:** `.run(closure)` is on `App` (use `.build(context).expect(...).run(...)`); `RunEvent::ExitRequested{code, api}`; `WindowEvent::Resized(PhysicalSize<u32>)` / `Moved(PhysicalPosition<i32>)`. `WebviewWindowBuilder::inner_size/position` take logical f64 values — for exact physical placement use `window.set_position(PhysicalPosition)` after build.

## 4. Backend Commands (in `src/lib.rs`)
`list_dir`, `home_dir`, `make_dir`, `rename_path`, `delete_path`, `delete_path_progress` (async with progress), `copy_path`, `move_path`, `link_path` (hard/soft link), `read_text_file`, `read_file_chunk`, `path_info`, `get_file_permissions`, `set_file_permissions` (POSIX rwx bits on Unix, read-only/archive/system/hidden attributes on Windows; async), `open_path`, `edit_path` (default editor: macOS `open -e`, Linux `$EDITOR`/`xdg-open`, Windows `ShellExecuteW` verb `"edit"`), `quit_app`, `get_appearance`, `set_theme`, `set_font`, `set_font_size`, `set_diff_command`, `set_diff_in_terminal`, `set_edit_command`, `set_edit_in_terminal`, `set_show_hidden`, `set_fuzzy_search`, `set_column_widths`, `get_favorites`, `set_favorites`, `get_fav_apps`, `set_fav_apps`, `open_settings`, `open_help`, `open_search`, `open_view`, `open_info`, `open_permissions` (open/center secondary windows), `search_files` (async, glob/regex, exclusions, recursive, content search), `copy_path_progress` / `move_path_progress` (async with per-file + overall progress), `get_copy_progress` (returns `Option<serde_json::Value>`), `cancel_copy`, `run_diff`, `run_edit`, `run_command`, `get_app_version`, `get_about_info`, `expand_env` (expands `%VAR%` on Windows, `$VAR`/`${VAR}` on Unix).
- `AppState.progress` is `HashMap<String, serde_json::Value>` — supports both `CopyProgress` and `DeleteProgress` structs.

## 5. Frontend Key Bindings (F-key bar)
F1 help (opens the help window), F2 diff (compare files), F3 view (opens the view window with lazy chunked loading), F4 edit, F5 copy, F6 move, F7 new folder, F8 delete, F9 change folder, F10 permissions (opens the POSIX permissions window), F11 rename, F12 info (opens the info window). The toolbar Quit button exits the application. Toolbar: Търсене opens the search window; Настройки opens the settings window. The F5 copy dialog shows hardlink/softlink checkboxes and a progress dialog with two bars (current file + overall).

**Standard clipboard (Ctrl/Cmd):** Ctrl/Cmd+C copy, Ctrl/Cmd+X cut, Ctrl/Cmd+V paste, Ctrl/Cmd+A select all, Ctrl/Cmd+D favorites, Ctrl/Cmd+S fav apps. Ctrl/Cmd+V uses the same `copyOrMove` flow with progress as F5/F6.
**Navigation:** Ctrl/Cmd+↓ change folder, Ctrl/Cmd+R run command, Ctrl/Cmd+←/→ open dir in other panel, Ctrl/Cmd+U swap panels.
**Shift selection:** Shift+Click, Shift+ArrowUp/Down, Shift+Home/End — range selection from anchor.
**Platform modifier:** `isMac` checks `navigator.platform`/`navigator.userAgent`; `mod = isMac ? ev.metaKey : ev.ctrlKey`.

**IMPORTANT:** When adding or changing key bindings, update BOTH the `help.text` arrays in `web/i18n.js` (bg + en) to reflect the current shortcuts.

## 6. Project Commands
- **System PATH (required for build commands):** `export PATH="/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/Users/fi4o/.cargo/bin:$PATH"` — must be set before `cargo`/`ditto`/`node` etc. on macOS.
- **Xcode toolchain:** If `xcrun`, `cc`, or SDK errors occur, ask the user to fix it outside — do NOT attempt automatic fixes.
- **Run Dev Mode:** `cargo tauri dev`
- **Build Production:** `cargo tauri build` (bundle appears under `target/release/bundle/`)
- **Lint Rust:** `cargo clippy`
- **Format Rust:** `cargo fmt`
- **macOS install after build:** `ditto "target/release/bundle/macos/p2manager.app" "/Applications/p2manager.app"`
- **Verify JS:** `node --check web/<file>.js`
- **Version:** bump BOTH `Cargo.toml` and `tauri.conf.json` `version`.

## 7. Coding Conventions & Safety
- **Tauri commands:** return `Result<T, String>` with human-readable errors so the frontend can `alert()`/display them.
- **fs-heavy Tauri commands MUST be `async`** — wrap fs work in `tauri::async_runtime::spawn_blocking`. Sync commands run on the main thread on macOS (WKWebView IPC) and freeze the whole UI on slow/network filesystems (NFS, autofs, systemd automount). `move_path_progress` tries `fs::rename` first, then falls back to progress-reporting copy+delete (cross-device/EXDEV).
- **Style:** spaces only, 4 spaces = 1 tab. Match surrounding code; no decorative comments.
- **WebKit Bug (macOS):** WKWebView can fire spurious `click` events, e.g. when a drag-select ends on a different element. Use `mousedown` for dismiss/tab/single-press UI (overlay backdrops, modal buttons) and avoid relying on `click` for anything near selectable text. Checkboxes use `change`.
- **No new Rust crates or JS packages** unless explicitly requested.


## 8. Modal Focus Rules
- Tab/Shift+Tab while a modal is open must cycle ONLY through visible focusable elements inside the active modal (`.modal`), never background controls. Implemented via `getModalFocusables()` + the `keydown` handler; `preventDefault()` on Tab.
- Restore focus to the element that had focus before opening the modal (`lastFocused`) when it closes.
- All text in modals comes from `t()` / `tp()` — never hardcode strings.
- Nested modals: save/restore `modalDismissCb` (`const prevDismiss = modalDismissCb`) before setting a new one, restore after resolve. This prevents inner modals (e.g. `promptModal` inside `favModal`) from clobbering the outer dismiss callback.
- **IMPORTANT:** `resolve()` MUST be called BEFORE `closeModal()`. `closeModal()` triggers `modalDismissCb` which also resolves the promise (with null/false). First resolve wins — if `closeModal()` runs first, the intended resolve value is lost. Pattern: `resolve(value); closeModal();`

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
