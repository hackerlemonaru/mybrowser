# MyBrowser — From-Scratch Rust Browser with Chrome-Like Profiles

A web browser built in Rust **from scratch** (no Chromium, no WebKit, no Tauri
wrapper), with a Chrome-style profile system for workload isolation, enterprise-grade
security, lightweight memory usage, and smooth 60fps rendering.

## Architecture

```
mybrowser/ (workspace)
├── engine/          Falco — from-scratch browser engine (HTML/CSS/JS/layout/paint)
│   ├── net/         HTTP/TLS, cookies, cache, proxy (per-profile)
│   ├── gpu/         Direct Vulkan renderer (ash — NOT WebGPU/wgpu)
│   ├── fingerprint/ Canvas, WebGL, Audio, Fonts, Screen spoofing (per-profile)
│   ├── security/    Site Isolation, CSP, Sandbox, Permissions
│   ├── memory/      Arena allocators, bounded caches (no leaks)
│   ├── performance/ Compositor thread, frame budgeting, damage tracking
│   └── window/      Interactive browser window (address bar, history, forms)
├── profile-manager/ Chrome-like profile manager (isolated data dirs per profile)
├── storage/         Per-profile localStorage, IndexedDB, history, bookmarks
├── extensions/      Chrome Manifest V3 extension system (sandboxed, per-profile)
├── gui/             Chrome-like profile switcher GUI (egui + winit)
└── browser/         The app — ties engine + profiles + extensions + proxy + fingerprint together
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

## Profile System (Chrome-like + Guest Mode)

Each profile gets its own isolated data directory:

```
~/.local/share/mybrowser/profiles/<profile-id>/
├── cookies/          → cookies.json (per-profile cookie jar)
├── cache/            → HTTP cache (per-profile)
├── history/          → browsing history
├── bookmarks/        → bookmarks
├── localstorage/     → localStorage per profile
├── indexeddb/        → IndexedDB per profile
├── extensions/       → extension storage per profile
└── sessions/         → session data
```

**Profile types:**
- **Regular** — persistent profile with name, avatar, theme color
- **Guest** — temporary profile, all data wiped when browser exits

Switching profiles swaps the active cookie jar, HTTP cache, localStorage,
IndexedDB, history, bookmarks, and extensions — zero session leakage between
Work, Personal, Banking, etc.

**Site Isolation:** Each `(profile, origin)` pair gets its own isolation context.
Work's `example.com` is completely separate from Personal's `example.com` —
never share cookies, storage, or cache.

## Chrome Extension Support (Manifest V3)

MyBrowser supports Chrome extensions with a **security-first sandbox** based on
the FIST BUMP security model:

| Security Feature | Implementation |
|-----------------|---------------|
| Process isolation | Extension host runs in separate process per profile |
| Content script isolation | Content scripts run in extension process, NOT renderer |
| DOM Proxy | Content scripts access page DOM through proxy, never direct references |
| Message validation | sender.id must match extension.id; no cross-extension messaging |
| No eval() | Dynamic code execution banned |
| No nativeMessaging | No host system access (banned) |
| Strict CSP | 'unsafe-eval' and 'unsafe-inline' banned |
| Allowlist actions | Only allowlisted privileged actions permitted |
| Per-profile storage | Extension data isolated per profile on disk |

Extensions are installed from .crx files or unpacked directories.

## Per-Profile Proxy

Each profile can route traffic through its own proxy:

```
Profile "Work"    → SOCKS5 proxy at 10.0.0.5:1080
Profile "Personal" → HTTP proxy at proxy.example.com:3128
Profile "Banking"  → Direct connection (no proxy)
```

Supports SOCKS5, SOCKS5h (remote DNS), and HTTP proxies with authentication.
Bypass list for local addresses.

## Fingerprint Spoofing (Anti-Detection)

Each profile gets a **consistent, internally-consistent fingerprint** that stays
the same across sessions:

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

Values are selected from real device profiles and kept internally consistent.

## Dummy System Profile (Privacy at Peak)

In Settings → Privacy Peak, enable **Dummy System Profile** to report completely
fake system information: OS, CPU, GPU, screen, timezone, locale — websites see
a completely different machine. Combine with per-profile proxy for IP isolation.

## Security Architecture

**Defense in depth** — multiple independent layers:

| Layer | Protection |
|-------|-----------|
| Site Isolation | Each (profile, origin) in separate context; cross-origin access denied |
| OS Sandbox | Renderers blocked from filesystem, network, exec |
| CSP | Restricts scripts, styles, images, connections per page |
| Extension Sandbox | FIST BUMP model; no eval, no nativeMessaging, sender validation |
| Memory Safety | Arena allocators per tab; O(1) free on close; bounded caches |
| Performance | Compositor thread; damage tracking; 16.67ms frame budget |

See `SECURITY.md` for full details.

## Performance

- **Compositor thread** — GPU rendering off main thread; 60fps even when JS blocks
- **Damage tracking** — only repaint changed regions; idle pages cost nothing
- **Frame budgeting** — 16.67ms target; metrics tracked per frame
- **Compositor-only animations** — transform, opacity on GPU thread
- **Lazy loading** — content-visibility: auto, loading="lazy" for images
- **Arena allocators** — per-tab memory, freed O(1) on close; no leaks

## Build & Run

Requires Rust (install via `rustup`) and Vulkan SDK (for GPU rendering).

```bash
cd mybrowser
cargo build --release

# Show the Chrome-like profile switcher GUI
cargo run -p profile-switcher

# Or launch directly with a profile ID
cargo run -p browser -- <profile_id> https://example.com

# Create profiles for different workloads
cargo run -p browser -- create Work
cargo run -p browser -- create Personal
cargo run -p browser -- create Banking

# Browse in an isolated context
cargo run -p browser -- <work-id> https://work-site.com
cargo run -p browser -- <personal-id> https://personal-site.com
```

## CLI Commands (browser crate)

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

## GUI Commands (profile-switcher crate)

- Click a profile card → Launch browser with that profile
- **"Add New"** → Create and launch a new regular profile
- **"Guest"** → Launch a temporary guest profile (wiped on exit)
- **"Settings"** → Configure dummy profile, extensions, proxy

## Repository

https://github.com/hackerlemonaru/mybrowser

## License

MIT
