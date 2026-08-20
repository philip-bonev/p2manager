# Project Context & Rules: Panel Manager 

## 1. System Prompt & Role
- **Role:** Expert software engineer specializing in Rust, Tauri, and frontend development. Provide clean, secure, idiomatic code.
- **Communication Language:** Always respond in Bulgarian unless asked otherwise; explain thought process and summarize code changes.
- **Code Language:** Rust backend in `/src`, vanilla HTML/CSS/JS frontend in `/web` (no framework, no build step).

## 2. Tech Stack & Architecture
- **Framework:** Tauri v2 (Rust backend + WebView frontend). Rust edition 2024.
- **Backend:** `/src/lib.rs` (single file, ~1340 lines).
- **Frontend:** static files in `/web`, served as `frontendDist: "web"`. There is NO npm/package.json — do not run `npm install`; edit files directly.
- **Config:** `tauri.conf.json`, `capabilities/default.json`, `Cargo.toml`.
- **Plugins:** opener, dialog, fs, window-state, single-instance, log.
- **Platform:** Windows, Linux, macOS.

## 3. Architecture & Data Flow (important)
- **Settings:** stored as JSON at `~/.p2manager/config.json` (cross-platform via HOME/USERPROFILE). Loaded in `.setup()` into `AppState.settings`.
- **Invoke arg naming:** JS passes camelCase keys (e.g. `newSettings`), Rust receives snake_case (`new_settings`) — Tauri maps automatically.

## 4. Project Commands
- **Run Dev Mode:** `cargo tauri dev`
- **Build Production:** `cargo tauri build` (bundle appears under `target/release/bundle/`)
- **Lint Rust:** `cargo clippy`
- **Format Rust:** `cargo fmt`
- **Verify JS:** `node --check web/<file>.js`
- **Version:** bump BOTH `Cargo.toml` and `tauri.conf.json` `version`.

## 5. Coding Conventions & Safety
- **Tauri commands:** return `Result<T, String>` with human-readable errors so the frontend can `alert()`/display them.
- **Style:** spaces only, 4 spaces = 1 tab. Match surrounding code; no decorative comments.
- **WebKit Bug (macOS):** WKWebView can fire spurious `click` events, e.g. when a drag-select ends on a different element. Use `mousedown` for dismiss/tab/single-press UI (overlay backdrops, properties tabs) and avoid relying on `click` for anything near selectable text. Checkboxes use `change`.
- **No new Rust crates or JS packages** unless explicitly requested.

## 6. Scoping
- Frontend logic/UI in `/web`, backend rust logic in `/src`. Do not mix concerns.
