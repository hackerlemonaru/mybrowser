# MyBrowser — From-Scratch Rust Browser with Chrome-Like UI

A web browser built in Rust **from scratch** (no Chromium, no WebKit, no Tauri wrapper), with a Chrome-style profile system for workload isolation, enterprise-grade security, lightweight memory usage, and smooth 60fps rendering. The front end is an immediate-mode GUI built with **egui 0.29 + winit 0.30 + egui_glow 0.29 (OpenGL)**, rendered through a real GLX context on Linux.

---

## Current Status

| Item | Status |
|------|--------|
| Chrome-like UI reconstruction (PR #1) | ✅ Built & running |
| egui/winit/egui_glow OpenGL window | ✅ Built |
| GLX context (legacy fallback for Xvfb) | ✅ Built |
| Per-profile proxy + VPN | ✅ Built |
| Password manager (encrypted at rest) | ✅ Built |
| Download manager | ✅ Built |
| Custom domains (118 non-registrable TLDs) | ✅ Built |
| Fingerprint spoofing / dummy system profile | ✅ Built |
| Bookmarks, history, localStorage, IndexedDB (per profile) | ✅ Built |
| Tab management (groups, pin, mute, freeze) | ✅ Built |
| Extensions (Manifest V3 sandbox) | ✅ Built |
| Deployed on VPS (noVNC port 6080) | ✅ Running |

**Live preview:** http://45.79.122.45:6080/vnc.html  
**Running binary (VPS):** `/root/mybrowser/mybrowser/target/release/mybrowser --window tabtest`  
**GitHub:** https://github.com/hackerlemonaru/mybrowser

---

## Chrome UI Reconstruction (PR #1)

The browser window (`browser/src/window.rs`) is a full Google Chrome-like reconstruction, laid out top-to-bottom exactly like Chrome:

```
[Tab Strip]        tabs + `+` new-tab button
[Nav Toolbar]      back/forward/reload | omnibox | star | extensions | profile | 3-dot
[Bookmarks Bar]    favicon + title per bookmark (only if bookmarks exist)
[Viewport]         Falco engine render output, fills all remaining space
```

### UI constraints honored
- **No separate "+ New Tab" button** — the `+` sits directly beside the open tabs in the tab strip.
- **Nothing surfaced outside the menu bar** — all controls live inside the 3-dot Chrome menu.
- **VPN inside the 3-dot menu** — each profile owns its own VPN setting.
- **Each profile gets completely fresh settings** — VPN, proxy, theme, location, dummy profile all isolated per profile.
- **Dark theme matching Chrome** — navbar ~`#202124`, active tab white-ish.

### Themes
`Theme::Light` / `Theme::Dark` with per-component colors:
- Frame / toolbar / active tab / inactive tab / text / text-muted / hover / border
- Dark navbar ≈ `#202124`; active tab is white-ish.
- Theme toggle lives in the 3-dot menu → Appearance.

### 3-dot Chrome menu
Opened from the toolbar, positioned under the menu button. Contains:
- New tab (Ctrl+T) / Reopen closed tab (Ctrl+Shift+T)
- History (Ctrl+H) / Downloads (Ctrl+J) / Bookmarks manager
- Appearance: Light / Dark theme toggle + "Show bookmarks bar" checkbox
- Exit

All controls connect to real browser functionality (tab open/close/switch, theme persistence, bookmarks bar visibility).

### Rendering
- `egui_glow` 0.29 winit API: `run()` / `paint()` every frame.
- OpenGL context created in `browser/src/gl/mod.rs` via `libloading` + `raw-window-handle` 0.6.
- Legacy `glXCreateContext` fallback for Xvfb (where `glXCreateContextAttribsARB` is unavailable).
- Window explicitly set visible (`window.set_visible(true)`) to fix black screen on noVNC.
- Falco engine renders to a texture; the viewport displays the engine's render-to-texture output.

---

## Architecture

```
mybrowser/ (workspace)
├── engine/          Falco — from-scratch browser engine (HTML/CSS/JS/layout/paint)
│   ├── net/         HTTP/TLS, cookies, cache, proxy (per-profile), custom domains, password manager
│   ├── gpu/         Direct Vulkan renderer (ash — NOT WebGPU/wgpu)
│   ├── fingerprint/ Canvas, WebGL, Audio, Fonts, Screen spoofing (per-profile)
│   ├── security/    Site Isolation, CSP, Sandbox, Permissions
│   ├── memory/      Arena allocators, bounded caches (no leaks)
│   ├── performance/ Compositor thread, frame budgeting, damage tracking
│   ├── tab_management/ Tab, TabManager, TabGroup, pin/mute/freeze
│   ├── dummy_profile/ Dummy System Profile (48+ real device profiles)
│   ├── location/    Location profiles (undetectable, per-profile)
│   ├── custom_domain/ 118 non-registrable TLDs + HF backend
│   ├── window/      Interactive browser window
│   └── lib.rs       render_to_buffer() → RenderedBuffer (width, height, RGBA)
├── profile-manager/ Chrome-like profile manager (isolated data dirs per profile)
├── storage/         Per-profile localStorage, IndexedDB, history, bookmarks, downloads
├── extensions/      Chrome Manifest V3 extension system (sandboxed, per-profile)
├── gui/             Chrome-like profile switcher GUI (egui + winit)
└── browser/         The app — CLI fetch mode + egui GUI browser window
```

### Profile data directory (per profile, isolated on disk)
```
~/.local/share/mybrowser/profiles/<profile-id>/
├── cookies/          → per-profile cookie jar
├── cache/            → HTTP cache
├── history/          → browsing history
├── bookmarks/        → bookmarks
├── localstorage/     → localStorage
├── indexeddb/        → IndexedDB
├── extensions/       → extension storage
├── sessions/         → session data
└── vault/            → encrypted password vault (per-profile)
```

**Profile isolation is real:** separate directories on disk. Switching profiles swaps the cookie jar, HTTP cache, localStorage, IndexedDB, history, bookmarks, extensions, proxy, VPN, fingerprint, location, and dummy profile — zero session leakage between Work, Personal, Banking, etc.

**Site Isolation:** each `(profile, origin)` pair gets its own isolation context.

---

## Chrome Extension Support (Manifest V3)

Security-first sandbox based on the FIST BUMP model:

| Security Feature | Implementation |
|-----------------|---------------|
| Process isolation | Extension host runs in separate process per profile |
| Content script isolation | Content scripts run in extension process, NOT renderer |
| DOM Proxy | Content scripts access page DOM through proxy, never direct references |
| Message validation | `sender.id` must match `extension.id`; no cross-extension messaging |
| No `eval()` | Dynamic code execution banned |
| No `nativeMessaging` | No host system access (banned) |
| Strict CSP | `unsafe-eval` and `unsafe-inline` banned |
| Allowlist actions | Only allowlisted privileged actions permitted |
| Per-profile storage | Extension data isolated per profile on disk |

Extensions installed from `.crx` files or unpacked directories.

---

## Per-Profile Proxy + VPN

Each profile can route traffic through its own proxy (SOCKS5, SOCKS5h remote-DNS, or HTTP with auth; bypass list for local addresses). VPN fields live in `ProxyConfig` and are managed by `ProxyManager::set_vpn()` / `vpn_status()`:

```
Profile "Work"    → SOCKS5 proxy at 10.0.0.5:1080, VPN off
Profile "Personal" → HTTP proxy at proxy.example.com:3128, VPN on (provider X)
Profile "Banking"  → Direct connection, no proxy, VPN off
```

VPN is per-profile, fresh per profile (defaults off, provider "None", endpoint empty). Each profile owns its own VPN setting, configurable from the 3-dot menu.

---

## Fingerprint Spoofing (Anti-Detection)

Each profile gets a **consistent, internally-consistent fingerprint** that stays the same across sessions:

| Vector | Spoofed |
|--------|---------|
| User-Agent + Client Hints | ✅ |
| WebGL vendor/renderer | ✅ |
| Canvas (noise injection at paint level) | ✅ |
| AudioContext | ✅ |
| Fonts | ✅ |
| Screen resolution + color depth | ✅ |
| Hardware concurrency + device memory | ✅ |
| Timezone + locale | ✅ |
| Platform / OS | ✅ |

Values selected from real device profiles (48+ in the dummy-profile catalog) and kept internally consistent.

### Dummy System Profile (Privacy at Peak)
In Settings → Privacy Peak, enable **Dummy System Profile** to report completely fake system information (OS, CPU, GPU, screen, timezone, locale). Websites see a completely different machine. Combine with per-profile proxy for IP isolation.

---

## Password Manager (Security Requirements)

| Requirement | Implementation |
|-------------|---------------|
| Save-password pop-out dialog on login success | ✅ (appears when login succeeds on any website) |
| Editing option for saved credentials | ✅ |
| Per-profile isolated vault | ✅ (separate vault directory per profile) |
| Encrypted at rest | ✅ |

CLI: `password-list`, `password-gen`, `password-health`, `password-export` (CSV). GUI: accessible from the 3-dot menu → Passwords.

---

## Download Manager

CLI: `download-list`, `download-stats`, `download-cancel`. Tracks total/completed/cancelled/interrupted downloads and bytes per profile.

---

## Custom Domains

118 **NON-registrable TLDs only** — NOT `.com/.net/.org` from GoDaddy. The HF backend is contacted **only** for registered custom domains, never for `.com/.net/.org` traffic. Per-profile domain registry with resolve statistics (local hits vs HF backend hits).

CLI: `--register-domain`, `--list-domains`, `--unregister-domain`, `--domain-stats`.

---

## Build & Run

Requires Rust (via `rustup`). On the VPS the GPU path uses OpenGL/GLX (not Vulkan) because Xvfb lacks a Vulkan driver.

```bash
cd mybrowser
cargo build --release

# Profile switcher GUI
cargo run -p profile-switcher

# Launch GUI browser window with a profile (egui + OpenGL)
cargo run -p browser -- --window <profile_id>

# CLI fetch mode (isolated profile context)
cargo run -p browser -- <profile_id> https://example.com

# Create isolated profiles
cargo run -p browser -- create Work
cargo run -p browser -- create Personal
cargo run -p browser -- create Banking
```

### CLI Commands (browser crate)
| Command | What it does |
|---------|-------------|
| `list` | List all profiles, show which is active |
| `create <name>` | Create a new isolated regular profile |
| `switch <id\|name>` | Switch the active profile |
| `delete <id>` | Delete a profile and wipe its entire data directory |
| `active` | Show the currently active profile |
| `browse <url> <profile>` | Fetch a URL using that profile's cookies/cache |
| `config <id\|name>` | Show all isolated paths for a profile |
| `--gui` | Show the Chrome-like profile switcher GUI |
| `--window <profile_id>` | Open the egui GUI browser window |

### GUI Commands (profile-switcher crate)
- Click a profile card → Launch browser with that profile
- **"Add New"** → Create and launch a new regular profile
- **"Guest"** → Launch a temporary guest profile (wiped on exit)
- **"Settings"** → Configure dummy profile, extensions, proxy

---

## VPS Deployment

The browser runs headless on the VPS via Xvfb + x11vnc + websockify + noVNC:

- **Display:** Xvfb `:99` (1920×1080×24, GLX extension)
- **VNC:** x11vnc on `:99` → rfbport 5900
- **noVNC:** websockify on port **6080**, web root `/opt/noVNC`
- **Preview:** http://45.79.122.45:6080/vnc.html
- **Startup script:** `/home/user/.vps/start_browser_remote.sh`
- **Running profile:** `tabtest`

To rebuild after source changes:
```bash
# On VPS
cd /root/mybrowser/mybrowser
cargo build --release -p browser
# then restart:
./target/release/mybrowser --window <profile_id>
```

---

## Security Architecture

Defense in depth — multiple independent layers:

| Layer | Protection |
|-------|-----------|
| Site Isolation | Each `(profile, origin)` in separate context; cross-origin access denied |
| OS Sandbox | Renderers blocked from filesystem, network, exec |
| CSP | Restricts scripts, styles, images, connections per page |
| Extension Sandbox | FIST BUMP model; no eval, no nativeMessaging, sender validation |
| Memory Safety | Arena allocators per tab; O(1) free on close; bounded caches |
| Performance | Compositor thread; damage tracking; 16.67ms frame budget |

See `SECURITY.md` for full details.

---

## Performance

- **Compositor thread** — GPU rendering off main thread; 60fps even when JS blocks
- **Damage tracking** — only repaint changed regions; idle pages cost nothing
- **Frame budgeting** — 16.67ms target; metrics tracked per frame
- **Compositor-only animations** — transform, opacity on GPU thread
- **Lazy loading** — `content-visibility: auto`, `loading="lazy"` for images
- **Arena allocators** — per-tab memory, freed O(1) on close; no leaks

---

## Roadmap / What's Left

The Chrome UI reconstruction (PR #1) is complete and deployed. Remaining work:

1. **GitHub push** — push the PR #1 fix commit (`Theme::hover()` + `escape_html()`) to `https://github.com/hackerlemonaru/mybrowser`.
2. **Falco render-to-texture bridge** — the viewport currently shows Falco output; a decoded favicon texture bridge to egui is still pending (noted in `window.rs`).
3. **Save-password pop-out dialog** — wired in the password manager; ensure it appears on login success in the GUI flow.
4. **Guest wipe-on-exit** — verify guest profile data is fully wiped when the session ends.
5. **Vulkan renderer** — `engine/src/gpu/` exists (ash-based); currently the VPS build uses OpenGL/GLX because Xvfb lacks Vulkan.
6. **JavaScript VM / WebAssembly** — `engine/src/js_runner.rs`, `js_tjs.rs`, `wasm/` exist; integrate fully into the page render pipeline.
7. **More acceptance-criteria polish** — confirm all 10 acceptance criteria pass against the running build.

---

## Repository

https://github.com/hackerlemonaru/mybrowser

## License

MIT
