# P2Manager (Panel Manager)

A fast, keyboard-driven **two-panel file manager** for Windows, Linux and macOS,
built with [Tauri v2](https://tauri.app) — a Rust backend and a plain
HTML/CSS/JS frontend served directly from the repository (no npm, no bundler,
no framework).

Classic Norton Commander–style workflow: two independent panels, function
keys F1–F12, clipboard copy/move, pattern selection, favorites, built-in file
search, diff and external editor integration — all wrapped in a modern,
themeable UI.

## Features

- **Dual panels** — independent directories, active-panel indicator,
  resizable columns (name / extension / size / date), swap panels (Ctrl+U),
  open selected directory in the other panel (Ctrl+←/→).
- **File operations** — copy, move, rename, delete, mkdir, hard/soft links,
  with **per-file and overall progress** bars and cancel support.
- **Selection** — mark/unmark by pattern (+/-), inverse selection (*),
  Shift+Click / Shift+Arrow range selection, Ctrl+A select all.
- **File search window** — glob or regex file name matching plus **content
  (grep) search**, recursive scan with exclusions; picking a result navigates
  the active panel to it.
- **Favorites** — favorite folders (Ctrl+D) and favorite applications (Ctrl+S).
- **Diff & edit** — F2 compares the selected files (external or terminal diff
  command, configurable), F3 text viewer, F4 opens files in the default or a
  custom editor (supports `%1`/`%2` placeholders and environment variables).
- **Settings window** — theme (follow system / light / dark), font, font size,
  show hidden files, fuzzy search, diff and edit commands; window geometry and
  column widths are restored on next start.
- **Help window** — keyboard reference and license text.
- **i18n** — Bulgarian and English UI, chosen automatically from the browser
  language.
- **Config** — plain JSON at `~/.p2manager/config.json` (uses `HOME` /
  `USERPROFILE`, cross-platform).

## Architecture

| Layer     | Location        | Notes                                                        |
|-----------|-----------------|--------------------------------------------------------------|
| Backend   | `src/lib.rs`    | Single Rust file, all Tauri commands, ~1900 lines            |
| Frontend  | `web/`          | `index.html`/`main.js`/`styles.css` (main window) plus `settings`, `help` and `search` windows, each with its own html/js/css |
| Shared    | `web/common.css`, `web/appearance.js`, `web/i18n.js` | Themes, form widgets, window chrome; theme/font sync; translations |
| Config    | `tauri.conf.json`, `capabilities/default.json`, `Cargo.toml` | Window definitions and permissions |

Secondary windows (`settings`, `help`, `search`) are created at runtime via
`open_settings` / `open_help` / `open_search`, centered on the main window.
Closing the main window exits the application (and closes all secondary
windows); closing a secondary window keeps the app running.

There is **no build step for the frontend** — files in `web/` are served as-is
via `frontendDist: "web"`. Edit them directly.

## Requirements (mandatory)

### Common

| Tool           | Version | Install                                                       |
|----------------|---------|---------------------------------------------------------------|
| Rust toolchain | **1.85+** (edition 2024) | https://rustup.rs                     |
| Tauri CLI      | **2.x** | `cargo install tauri-cli --locked` (provides `cargo tauri`)   |

**Node.js / npm are NOT required** — the frontend has no dependencies and no
build step. (`node` is only optional, used to run `node --check web/*.js` as a
syntax check.)

### macOS

- Xcode Command Line Tools: `xcode-select --install`
  (provides `clang`, `cc`, SDK). If `xcrun`/`cc` errors appear, fix the
  toolchain first — the project does not work around them.

### Windows

- Rust **MSVC** toolchain (installed by rustup on default).
- **Microsoft C++ Build Tools** (Visual Studio Build Tools with the
  *Desktop development with C++* workload).
- **WebView2 Runtime** — preinstalled on current Windows 10/11; otherwise
  download from Microsoft.

### Linux

Debian / Ubuntu:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Fedora:

```bash
sudo dnf install gtk3-devel webkit2gtk4.1-devel openssl-devel curl file \
    libappindicator-gtk3-devel librsvg2-dev
```

Arch:

```bash
sudo pacman -S webkit2gtk-4.1 base-devel file openssl \
    appindicator-gtk3 librsvg
```

## Building

```bash
# Development (hot reload)
cargo tauri dev

# Production build (release binary + platform bundles)
cargo tauri build
```

Bundles are written to `target/release/bundle/` (`targets: "all"`):

- **macOS** — `.app` / `.dmg`
- **Windows** — `.msi` / NSIS installer
- **Linux** — `.deb` / AppImage

Quality checks:

```bash
cargo clippy          # lint (no warnings from project code)
cargo fmt             # format Rust
node --check web/<file>.js   # optional JS syntax check
```

Note: the frontend directory must stay at `web/` — it is referenced as
`frontendDist` in `tauri.conf.json`.

## Keyboard shortcuts

| Keys | Action |
|------|--------|
| ↑/↓, Enter, Backspace, Tab, Home/End | Navigate, enter directory, switch panels, first/last |
| Space / `+` / `-` / `*` | Mark, mark by pattern, unmark, invert |
| Ctrl/Cmd+C / X / V | Clipboard copy / cut / paste (with progress) |
| Ctrl/Cmd+A / D / S | Select all / favorite folders / favorite apps |
| Ctrl/Cmd+U / ←/→ / ↓ / R | Swap panels / open in other panel / change folder / run command |
| F1 | Help window |
| F2 | Diff the selected files of both panels |
| F3 / F4 | View text file / edit in default editor |
| F5 / F6 | Copy / move to the other panel |
| F7 / F8 / F11 | New folder / delete / rename |
| F9 / F10 | Change folder / quit |
| F12 | Info about the selected item |

Settings, Help (F1) and the file search window open from the toolbar as
separate windows.

The full list lives in the in-app help window (F1) and in `web/i18n.js`
(`help.text`, both `bg` and `en`).

## License

Apache License 2.0 — see [LICENSE](LICENSE).
