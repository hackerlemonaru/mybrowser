# MyBrowser — From-Scratch Rust Browser with Chrome-Like Profiles

A web browser built in Rust **from scratch** (no Chromium, no WebKit, no Tauri
wrapper), with a Chrome-style profile system for workload isolation.

## Architecture

```
mybrowser/ (workspace)
├── engine/          Falco — from-scratch browser engine (HTML/CSS/JS/layout/paint)
├── profile-manager/ Chrome-like profile manager (isolated data dirs per profile)
└── browser/         The app — ties engine + profiles together
```

## Why "from scratch"?

Most "Rust browsers" are wrappers around:
- **Tauri** → uses the OS webview (WebKitGTK / WebView2 / WKWebView)
- **Electron** → bundles Chromium
- **Servo** → embeddable but still a separate project you depend on

This project builds on **Falco** (`engine/`), a ~73,000-line browser engine where
every module — HTML tokenizer, CSS parser, DOM, layout, paint, JavaScript VM,
WebAssembly interpreter, PNG encoder — is written from scratch in Rust. No
imported engine, no WebView, no Chromium.

## Profile System (Chrome-like)

Each profile gets its own isolated data directory:

```
~/.local/share/mybrowser/profiles/<profile-id>/
├── cookies/          → cookies.json (per-profile cookie jar)
├── cache/            → HTTP cache (per-profile)
├── history/          → browsing history
├── bookmarks/        → bookmarks
├── extensions/       → extension storage
├── localstorage/     → localStorage per profile
└── sessions/         → session data
```

Switching profiles swaps the active cookie jar, HTTP cache, and storage — zero
session leakage between Work, Personal, Banking, etc.

## Build & Run

Requires Rust (install via `rustup`).

```bash
cd mybrowser
cargo build --release

# Create profiles for different workloads
cargo run -p browser -- create Work
cargo run -p browser -- create Personal
cargo run -p browser -- create Banking

# Switch into a workload
cargo run -p browser -- switch Work

# Browse a URL inside a profile's isolated context
cargo run -p browser -- browse https://example.com Work

# See the active profile and its isolated paths
cargo run -p browser -- active
cargo run -p browser -- config Work
```

## CLI Commands

| Command | What it does |
|---------|-------------|
| `list` | List all profiles, show which is active |
| `create <name>` | Create a new isolated profile with its own data dir |
| `switch <id\|name>` | Switch the active profile |
| `delete <id>` | Delete a profile and wipe its entire data directory |
| `active` | Show the currently active profile |
| `browse <url> <profile>` | Fetch a URL using that profile's cookies/cache |
| `config <id\|name>` | Show all isolated paths for a profile |

## What's Built

- ✅ ProfileManager: create/list/switch/delete profiles with isolated data dirs
- ✅ Per-profile cookie jar (persisted to disk per profile)
- ✅ Per-profile HTTP cache (persisted to disk per profile)
- ✅ From-scratch engine: HTML parser, CSS cascade, layout (flex/grid/table), paint
- ✅ Custom JavaScript VM (TJS) with bytecode interpreter + JIT
- ✅ WebAssembly interpreter
- ✅ 34+ Web Platform APIs
- ✅ Interactive window mode (scroll, click, forms, navigation, address bar)

## What's Next

- GUI profile switcher in the browser window (not just CLI)
- Per-profile localStorage, IndexedDB, history, bookmarks persistence
- Per-profile extensions storage
- Per-profile proxy / exit IP (network-layer isolation beyond storage)
- Per-profile fingerprint spoofing

## Repository

https://github.com/hackerlemonaru/mybrowser
