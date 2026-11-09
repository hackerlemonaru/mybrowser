# MyBrowser — Rust starter with Chrome-like profile system

This is a **starter scaffold**, not a full browser. It implements the
profile-management core you asked for: isolated, switchable profiles
(each with its own cookies, history, bookmarks, extensions, and cache)
so you can manage separate workloads exactly like Chrome profiles.

## What's here

- `src/profile.rs` — `ProfileManager`: create / list / switch / delete
  profiles. Profiles are persisted to `profiles.json` under the platform
  data dir (`~/.local/share/mybrowser/profiles` on Linux).
- `src/browser.rs` — `BrowserConfig`: per-profile configuration that
  points the browser engine at the active profile's isolated data dir.
- `src/main.rs` — CLI: `list`, `create <name>`, `switch <id|name>`,
  `delete <id>`, `active`, `config <id|name>`.

## Build & run

```bash
cd mybrowser
cargo build --release

# Create profiles for different workloads
cargo run -- create Work
cargo run -- create Personal
cargo run -- create Banking

# Switch into a workload
cargo run -- switch Work

# See the active profile and its config (data dir, cookie path, etc.)
cargo run -- active
cargo run -- config Work
```

## Turning this into a real browser

The missing piece is the **rendering engine**. Writing one from scratch
is a multi-year project; instead, embed an existing engine:

| Approach | Crate / tooling | Notes |
|----------|----------------|-------|
| System webview | [tauri](https://tauri.app) / webkit2gtk / webview2 | Easiest. Native OS webview (Safari/WebKit on macOS, Edge/WebView2 on Windows, WebKitGTK on Linux). |
| Servo | [servo](https://servo.org/) | Rust-native browser engine. Embeddable but still maturing. |
| Gecko / Chromium | `mozilla-central` / Chromium Embedded | Powerful, but complex C++ FFI bindings. |

### Suggested architecture

```
main.rs (Rust)
  └─ ProfileManager  ← picks active profile dir
       └─ BrowserConfig
            └─ Webview (Tauri / WebKitGTK)
                 └─ user_data_dir = active profile's data dir
```

When the user switches profiles, destroy the old webview and spawn a
new one pointed at the new `user_data_dir`. Because every profile has a
separate directory on disk, cookies, `localStorage`, sessions, and
extension state are fully isolated — no cross-profile leakage.

### Next steps

1. Add a UI (Tauri is the most natural Rust choice for a webview browser).
2. Wire a navigation bar + address bar that talks to the webview.
3. Implement per-profile storage adapters (cookies, history, bookmarks)
   reading/writing under each profile's data dir.
4. Add profile switching in the UI that restarts the webview with the
   new `user_data_dir`.

## License

MIT — hack on it freely.