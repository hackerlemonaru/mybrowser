# MyBrowser — Agent Onboarding / Repo Map

**Purpose of this file:** If a new agent opens this repo, this file tells them exactly what the project is, what is done, what is left, where every file lives and what it does, how to build/run, and the hard constraints that must never be violated.

---

## 1. What this project is

**MyBrowser** is a web browser built in Rust **from scratch** — no Chromium, no WebKit, no Tauri wrapper, no OS webview. It has a Chrome-like profile system for workload isolation, enterprise-grade security, and smooth 60fps rendering.

The front end is an immediate-mode GUI built with **egui 0.29 + winit 0.30 + egui_glow 0.29 (OpenGL)**, rendered through a real GLX context on Linux.

- **Engine:** `engine/` = Falco, a from-scratch browser engine (HTML tokenizer, CSS parser, DOM, layout, paint, JS VM, WASM interpreter, PNG encoder).
- **GUI shell:** `browser/` = egui + winit browser window (tabs, nav toolbar, bookmarks bar, viewport, 3-dot Chrome menu).
- **Profiles:** `profile-manager/` + `storage/` = per-profile isolated data dirs on disk.
- **Extensions:** `extensions/` = Manifest V3 sandbox.
- **Switcher GUI:** `gui/` = Chrome-like profile switcher.

**Repo:** https://github.com/hackerlemonaru/mybrowser  
**Branch:** `main`  
**Workspace root:** `/home/user/mybrowser`

---

## 2. What is DONE (complete)

| Area | Status | Notes |
|------|--------|-------|
| Chrome-like UI reconstruction (PR #1) | ✅ Done | `browser/src/window.rs` rebuilt & running |
| egui/winit/egui_glow OpenGL window | ✅ Done | `run()`/`paint()` every frame |
| GLX context (legacy fallback for Xvfb) | ✅ Done | `browser/src/gl/mod.rs` |
| Per-profile proxy + VPN | ✅ Done | `engine/src/net/proxy.rs` |
| Password manager (encrypted at rest) | ✅ Done | `engine/src/net/password_manager/` |
| Download manager | ✅ Done | `storage/src/download/` |
| Custom domains (118 non-registrable TLDs) | ✅ Done | `engine/src/custom_domain/` |
| Fingerprint spoofing + dummy system profile | ✅ Done | `engine/src/fingerprint/`, `engine/src/dummy_profile/` |
| Bookmarks, history, localStorage, IndexedDB (per profile) | ✅ Done | `storage/src/` |
| Tab management (groups, pin, mute, freeze) | ✅ Done | `engine/src/tab_management/` |
| Extensions (Manifest V3 sandbox) | ✅ Done | `extensions/` |
| Deployed on VPS via noVNC | ✅ Running | Port 6080 |

**Live preview (VPS):** http://45.79.122.45:6080/vnc.html  
**Running binary (VPS):** `/root/mybrowser/mybrowser/target/release/mybrowser --window tabtest`

---

## 3. What is LEFT (roadmap)

1. **GitHub push** — push the latest commits (README update + window.rs fix) to `main`.
2. **Falco render-to-texture bridge** — decoded favicon texture bridge to egui is pending (noted in `window.rs`).
3. **Save-password pop-out dialog** — ensure it appears on login success in the GUI flow.
4. **Guest wipe-on-exit** — verify guest profile data fully wiped when session ends.
5. **Vulkan renderer** — `engine/src/gpu/` exists (ash-based); VPS build currently uses OpenGL/GLX (Xvfb lacks Vulkan).
6. **JavaScript VM / WebAssembly** — `js_runner.rs`, `js_tjs.rs`, `wasm/` exist; integrate fully into page render pipeline.
7. **Acceptance-criteria polish** — confirm all 10 acceptance criteria pass against the running build.

---

## 4. The 10 acceptance criteria (still apply)

1. Tab strip at top with `+` beside open tabs (no separate "+ New Tab" button).
2. Nav toolbar below the tab strip.
3. Conditional bookmarks bar (only if bookmarks exist).
4. Viewport with Falco render-to-texture.
5. Light/dark themes (dark navbar ~`#202124`, active tab white-ish).
6. 3-dot Chrome menu (everything inside the menu bar).
7. VPN inside the 3-dot menu.
8. Each profile owns its own VPN setting.
9. Each profile gets completely fresh settings.
10. No controls surfaced outside the menu bar.

---

## 5. Hard constraints (never violate)

- **GitHub token:** stored in session context (never echo, never commit). Used only for push/storage to `hackerlemonaru/mybrowser`.
- **HF token:** saved to `/home/user/.vps/hf_token.env` (chmod 600, temp use only, never echo, never commit).
- **Profile isolation must be real** — separate directories on disk, zero session leakage.
- **Extensions:** no `nativeMessaging`, no `eval`/dynamic code, content scripts isolated from page JS, message passing validated by `sender.id`.
- **Do not disturb existing working things on VPS** — only install/build in dedicated directories.
- **Custom domains:** 118 NON-registrable TLDs only — NOT `.com/.net/.org` from GoDaddy. HF backend contacted only for registered custom domains.
- **Location profiles:** must be perfect and undetectable — every signal consistent, no mismatches.
- **Password manager:** save-password pop-out on login success; editing option for saved credentials; per-profile isolated vault; credentials encrypted at rest.
- **Chrome UI:** no separate "+ New Tab" button — `+` beside open tabs. Everything inside the 3-dot menu. VPN inside 3-dot menu. Each profile owns its VPN setting. Fresh settings per profile. Dark navbar ~`#202124`.

---

## 6. Complete file structure & what each file does

```
mybrowser/                    <- workspace root (Cargo workspace)
├── Cargo.toml                <- workspace root: members = engine, profile-manager, storage, gui, browser
├── README.md                 <- user-facing project README (features, build, roadmap)
├── AGENT_ONBOARDING.md       <- THIS FILE: what's done, what's left, file map, constraints
├── SECURITY.md               <- full security architecture details
├── .gitignore
│
├── browser/                  <- THE APP: ties engine + profiles + extensions + proxy + fingerprint together
│   ├── Cargo.toml            <- egui 0.29, egui-winit 0.29, egui_glow 0.29, winit 0.30, glow 0.14, raw-window-handle 0.6, libloading 0.8
│   └── src/
│       ├── main.rs           <- CLI fetch mode + custom domain + download/password manager CLI + egui_glow OpenGL renderer + gl module import
│       ├── window.rs         <- PR #1 Chrome UI reconstruction: tab strip, nav toolbar, bookmarks bar, viewport, themes, 3-dot menu, VPN, dummy/location panels
│       └── gl/
│           └── mod.rs        <- GLX context via libloading + raw-window-handle 0.6; legacy glXCreateContext fallback for Xvfb
│
├── engine/                   <- Falco: from-scratch browser engine (crate name "falco")
│   ├── Cargo.toml
│   ├── README.md
│   ├── LICENSE
│   ├── docs/falco.svg
│   └── src/
│       ├── lib.rs            <- render_to_buffer() -> RenderedBuffer (width, height, RGBA); RenderOptions
│       ├── main.rs
│       ├── window.rs         <- interactive browser window (address bar, history, forms)
│       ├── multiprocess.rs
│       ├── js_runner.rs      <- JavaScript runner
│       ├── js_tjs.rs         <- JS engine (tjs)
│       ├── tjs/              <- TJS JS engine source
│       ├── tjs_ext/
│       ├── wasm/             <- WebAssembly interpreter
│       ├── png/              <- PNG encoder (from scratch)
│       ├── svg/
│       ├── image/
│       ├── media/
│       ├── memory/           <- arena allocators, bounded caches
│       ├── performance/      <- compositor thread, frame budgeting, damage tracking
│       ├── security/         <- Site Isolation, CSP, Sandbox, Permissions
│       ├── gpu/              <- Direct Vulkan renderer (ash — NOT WebGPU/wgpu)
│       ├── net/
│       │   ├── mod.rs        <- fetch(), fetch_bytes(), cookies, cache, redirects, proxy
│       │   ├── proxy.rs      <- Per-profile proxy; ProxyConfig + ProxyManager.set_vpn() / vpn_status()
│       │   ├── password_manager/mod.rs  <- PasswordManager; encrypted vault; health; export CSV
│       │   ├── custom_domain/        <- 118 non-registrable TLDs + HF backend client (hf_client.rs)
│       │   └── ...
│       ├── fingerprint/      <- Canvas, WebGL, Audio, Fonts, Screen spoofing (per-profile)
│       ├── dummy_profile/    <- Dummy System Profile (48+ real device profiles); ui/ selector
│       ├── location/         <- Location profiles (undetectable, per-profile); ui/ selector
│       ├── tab_management/mod.rs  <- Tab, TabManager, TabGroup, pin/mute/freeze, animations
│       ├── html/             <- HTML tokenizer
│       ├── css/              <- CSS parser (spec: cascade, selectors, advanced)
│       ├── dom/              <- DOM (spec)
│       ├── layout/           <- layout engine
│       ├── paint/            <- paint engine
│       ├── style/            <- style system
│       ├── web_api/
│       ├── web_runtime/
│       ├── wpt_runner.rs     <- WPT test runner
│       └── ...
│
├── profile-manager/          <- Chrome-like profile manager
│   ├── Cargo.toml
│   └── src/lib.rs            <- ProfileManager; resolve(profile_id) -> profile with data_dir; ProfileType
│
├── storage/                  <- Per-profile localStorage, IndexedDB, history, bookmarks, downloads
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs            <- ensure_dir/read_json/write_json helpers; StorageKind enum
│       ├── bookmarks.rs      <- Bookmark struct + add/list/remove per profile
│       ├── history.rs
│       ├── local_storage.rs
│       ├── indexed_db.rs
│       └── download/         <- download list/stats/cancel per profile
│
├── gui/                      <- Chrome-like profile switcher GUI (egui + winit)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           <- profile manager app entry
│       ├── lib.rs            <- ProfileSwitcher GUI
│       └── settings.rs       <- Settings panel UI
│
└── extensions/               <- Chrome Manifest V3 extension system (sandboxed, per-profile)
    └── Cargo.toml (+ src)
```

### Key source files a new agent should read first

| File | Why it matters |
|------|---------------|
| `browser/src/main.rs` | Entry point. Modes: `--gui` (switcher), `--window <profile>` (egui browser window), `<profile> [url]` (CLI fetch). Custom domain / download / password CLI handlers. |
| `browser/src/window.rs` | The Chrome UI reconstruction. Tab strip, nav toolbar, bookmarks bar, viewport, themes (`Theme::Light`/`Theme::Dark` with `hover()`, `frame()`, `toolbar()`, etc.), 3-dot menu, VPN per profile, dummy/location side panels. |
| `browser/src/gl/mod.rs` | OpenGL/GLX context creation for Linux via `libloading` + `raw-window-handle`. Legacy fallback for Xvfb. |
| `engine/src/net/proxy.rs` | `ProxyConfig` (incl. VPN fields), `ProxyManager` with `set_config`, `get_config`, `set_vpn`, `vpn_status`. |
| `engine/src/lib.rs` | `render_to_buffer()` -> `RenderedBuffer`; `RenderOptions` (width/height/background/animation_time_ms). |
| `engine/src/tab_management/mod.rs` | `Tab`, `TabManager` (open/close/switch/get_tab/tab_order/active_tab), `TabGroup`, `TabState`, animations. |
| `storage/src/bookmarks.rs` | `Bookmark` struct + `add`/`list`/`remove` per profile dir. |
| `profile-manager/src/lib.rs` | `ProfileManager::resolve(profile_id)` returning profile with `data_dir`. |
| `gui/src/lib.rs` / `gui/src/settings.rs` | Profile switcher GUI and settings panel. |
| `engine/src/net/custom_domain/mod.rs` + `hf_client.rs` | Custom domain registry; HF backend only for non-registrable TLDs. |
| `engine/src/net/password_manager/mod.rs` | Password vault; encrypted at rest; health check; CSV export. |

---

## 7. How to build & run

Requires Rust (via `rustup`). On the VPS the GPU path uses OpenGL/GLX (Xvfb lacks Vulkan).

```bash
cd mybrowser

# Full release build
cargo build --release

# Chrome-like profile switcher GUI
cargo run -p profile-switcher

# Launch egui GUI browser window with a profile (opens window)
cargo run -p browser -- --window <profile_id>

# CLI fetch mode (isolated profile context, fetch and exit)
cargo run -p browser -- <profile_id> https://example.com

# Create isolated profiles
cargo run -p browser -- create Work
cargo run -p browser -- create Personal
cargo run -p browser -- create Banking
```

### CLI commands (browser crate)

| Command | What it does |
|---------|-------------|
| `list` | List all profiles, show active |
| `create <name>` | Create isolated regular profile |
| `switch <id\|name>` | Switch active profile |
| `delete <id>` | Delete profile + wipe its data dir |
| `active` | Show active profile |
| `browse <url> <profile>` | Fetch URL using profile's cookies/cache |
| `config <id\|name>` | Show all isolated paths for a profile |
| `--gui` | Profile switcher GUI |
| `--window <profile_id>` | egui GUI browser window |
| `--register-domain <domain> --serving-url <url>` | Register custom domain for profile |
| `--list-domains` / `--unregister-domain` / `--domain-stats` | Custom domain management |
| `--download-list` / `--download-stats` / `--download-cancel <id>` | Download manager |
| `--password-list` / `--password-gen` / `--password-health` / `--password-export` | Password manager |

### GUI commands (profile-switcher crate)
- Click a profile card → launch browser with that profile
- **"Add New"** → create and launch a new regular profile
- **"Guest"** → temporary guest profile (wiped on exit)
- **"Settings"** → configure dummy profile, extensions, proxy

---

## 8. VPS deployment (headless, noVNC)

The browser runs on the VPS via Xvfb + x11vnc + websockify + noVNC:

| Component | Detail |
|-----------|--------|
| Display | Xvfb `:99`, 1920×1080×24, GLX extension |
| VNC | x11vnc on `:99` → rfbport 5900 |
| noVNC | websockify on port **6080**, web root `/opt/noVNC` |
| Preview URL | http://45.79.122.45:6080/vnc.html |
| Startup script | `/home/user/.vps/start_browser_remote.sh` |
| Running profile | `tabtest` |
| Running binary | `/root/mybrowser/mybrowser/target/release/mybrowser --window tabtest` |

### Rebuild + restart on VPS (after source changes)
```bash
# On VPS
cd /root/mybrowser/mybrowser
cargo build --release -p browser
pkill -f 'mybrowser --window'   # stop old instance
./target/release/mybrowser --window <profile_id>   # restart
```

### VPS helper scripts (workspace)
- `/home/user/.vps/vps.env` — VPS connection details
- `/home/user/.vps/vps.json` — VPS details in JSON
- `/home/user/.vps/vps.py` — Python pty-based SSH/SCP helper
- `/home/user/.vps/hf_token.env` — HF token (chmod 600)
- `/home/user/.vps/start_browser_remote.sh` — Path B startup (Xvfb + mybrowser + x11vnc + websockify + noVNC on 6080)

---

## 9. GitHub / repo info

- Remote: `https://github.com/hackerlemonaru/mybrowser`
- Branch: `main`
- Last commit: `3d2f134` Clean up unused code in window.rs
- To push local changes:
  ```bash
  cd mybrowser
  git add -A
  git commit -m "<message>"
  git push origin main
  ```
  (Use the GitHub token for authentication; never echo it.)

---

## 10. Quick orientation for a new agent

1. **Start here:** `browser/src/main.rs` (entry point) + `browser/src/window.rs` (the UI).
2. **The engine:** `engine/src/lib.rs` for rendering; `engine/src/net/` for network/proxy/password/custom-domains.
3. **Profiles:** `profile-manager/src/lib.rs` + `storage/src/` for per-profile isolated data.
4. **UI framework:** egui 0.29 + winit 0.30 + egui_glow 0.29; OpenGL via `browser/src/gl/mod.rs`.
5. **Where code lives:** everything under `mybrowser/`; do NOT touch unrelated files in `/home/user/` root (those are scratch/legacy).
6. **Constraints:** re-read section 5 before making changes — especially profile isolation, extension sandbox rules, custom-domain TLD rule, and the Chrome UI menu constraints.
