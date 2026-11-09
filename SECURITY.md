# MyBrowser Security Architecture

MyBrowser is built with **defense in depth** — multiple independent security layers so that a compromise in one layer cannot cascade to others.

---

## 1. Multi-Process Isolation (Chrome-style)

| Layer | Responsibility | Privilege |
|-------|--------------|-----------|
| **Browser Process** | UI, profile management, storage, networking, GPU orchestration | Privileged |
| **Renderer Process** (per profile+origin) | HTML/CSS/JS parsing, layout, paint | **Sandboxed** |
| **GPU Process** (direct Vulkan) | Compositor, display output | Privileged (isolated) |
| **Extension Host** (per extension) | Manifest V3 service worker, content scripts | **Sandboxed** |

Each profile runs its own renderer context. Each origin within a profile gets its own **isolation context**. Cross-origin data access is mediated by the browser process — never direct.

## 2. Site Isolation

Implemented in `engine/src/security/site_isolation.rs`:

- Each `(profile_id, origin)` pair gets a unique `IsolationContext`
- Cross-context access is **denied by default**
- Same-origin but different profiles never share storage, cookies, or cache
- Compromised renderer for `evil.com` cannot read `bank.com` data — even via Spectre-class side channels

```rust
// Example: Work profile's example.com is isolated from Personal's example.com
let work_ctx = isolation.create_context("profile-work", "https://example.com");
let personal_ctx = isolation.create_context("profile-personal", "https://example.com");
assert!(!isolation.can_access(&work_ctx, &personal_ctx)); // Always false
```

## 3. OS-Level Sandbox

Implemented in `engine/src/security/sandbox.rs`:

| Resource | Without Sandbox | With Strict Sandbox |
|----------|----------------|---------------------|
| Local filesystem | Accessible | Blocked (browser-mediated only) |
| Arbitrary TCP/UDP | Accessible | Blocked (fetch-only via browser) |
| Execute binaries | Accessible | Blocked |
| Other processes' memory | Accessible | Blocked |

Three levels: `None` (trusted local only), `Basic` (blocks exec/mount), `Strict` (only allowlisted syscalls).

Renderers run at `Strict` by default. All system access goes through the browser process, which validates origin and policy.

## 4. Content Security Policy (CSP)

Implemented in `engine/src/security/csp.rs`:

- Parses `Content-Security-Policy` headers
- Restricts script-src, style-src, img-src, connect-src, frame-src, font-src
- **Extensions must use strict CSP**: no `'unsafe-inline'`, no `'unsafe-eval'`
- `strict_extension_csp()` enforces Manifest V3 requirements

Extension CSP validation rejects any policy with `unsafe-inline` or `unsafe-eval`.

## 5. Extension Security (Manifest V3 + FIST BUMP model)

Implemented in `extensions/`:

| Threat | Mitigation |
|--------|-----------|
| Content script compromise → extension takeover | Content scripts run in **extension process, NOT renderer**; DOM Proxy mediates all DOM access |
| Cross-extension message hijacking | `sender.id` must match `extension.id`; foreign messages rejected |
| eval() / dynamic code | **Banned** — CSP blocks it, sandbox blocks it |
| nativeMessaging → host system access | **Banned** — manifest validation rejects it |
| Extension reads arbitrary URLs | Background handlers are **allowlisted operations only** — no generic URL fetch proxy |
| Extension storage leakage | Per-profile, per-extension storage on disk; extension A cannot read extension B's data |
| Content script trusts page data | All page-derived data treated as untrusted; validated before forwarding to background |
| Privileged actions from content scripts | Allowlist of actions only; sender.tab.id used (not message-claimed tab ID) |

### Extension message handler security pattern (from research):

```rust
// Only accept messages from this extension's own components
if message.extension_id != expected_id { reject; }
// Validate sender component type
match message.sender_component {
    ContentScript => { only allowlisted actions }
    Background => { full allowlisted actions }
}
// Scope effects to sender's tab only
if let ContentScript = component { require sender_tab_id; }
```

## 6. Permission System

Implemented in `engine/src/security/permissions.rs`:

- Per-profile, per-origin permission grants
- States: `Allow`, `Deny`, `Ask`
- Covers: Camera, Microphone, Geolocation, Notifications, Clipboard, Fullscreen, Storage, Bluetooth, USB, Serial
- Defaults to `Ask` — nothing is granted without user consent

## 7. Memory Safety & Leak Prevention

Implemented in `engine/src/memory/`:

### Arena Allocators (bumpalo)

- **PageArena** — one per tab/page lifetime
- All DOM nodes, CSS stylesheets, layout boxes, paint commands allocated in the arena
- When the tab closes, **entire arena freed in O(1)** — no per-object drop overhead
- No memory leaks across sessions

### Bounded Caches with TTL

| Cache | Max Entries | Max Size | TTL |
|-------|------------|----------|-----|
| HTTP cache | Configurable | Configurable | Per-response Cache-Control |
| Cookies | Configurable | — | Per-cookie max-age |
| Images | Configurable | Configurable | Fixed TTL |

When limits are reached, oldest entries are evicted. No unbounded growth.

### Object Pools

- Paint commands recycled instead of reallocated
- Reduces allocation pressure during rendering

## 8. Performance & Smoothness

Implemented in `engine/src/performance/`:

### Compositor Thread (GPU rendering off main thread)

```
Main Thread:        JS → Style → Layout → Paint → [commit]
                                                  ↓
Compositor Thread:                    Composite → GPU → Display
                                              ↑
                                              Scroll, transform, opacity handled here
```

- Compositor handles scrolling, transform, opacity animations **without main thread**
- Main thread can be blocked by JS and compositor still maintains 60fps
- `CompositorMsg::Scroll` — scroll handled entirely in compositor thread

### Frame Budgeting

- Target: **16.67ms per frame** (60fps)
- Metrics tracked: style, layout, paint, composite time
- Dropped frames counted and reported
- `FrameScheduler::should_yield()` — yields to compositor when budget exceeded

### Damage Tracking

- Only damaged rectangles are repainted (not the whole page)
- `DamageTracker` merges overlapping regions
- Idle pages cost nothing (no repaint when nothing changes)

### Compositor-Only Animations

| Property | Thread | Main Thread Work |
|----------|--------|-----------------|
| `transform` | Compositor | None |
| `opacity` | Compositor | None |
| `width`/`height` | Main | Layout + Paint + Composite |
| `top`/`left` | Main | Layout + Paint + Composite |

**Rule:** Only animate `transform` and `opacity` for smooth 60fps.

### Lazy Loading

- `content-visibility: auto` — skip layout/paint for off-screen content
- `loading="lazy"` for images — only load near viewport
- Critical rendering path: inline critical CSS, defer non-critical JS

---

## 9. Security Checklist for Extensions

Before enabling any extension, verify:

- [ ] Manifest V3 (no V2 background pages)
- [ ] No `nativeMessaging` permission
- [ ] No `<all_urls>` host permission unless strictly necessary
- [ ] CSP has no `'unsafe-inline'` or `'unsafe-eval'`
- [ ] No `eval()` or `new Function()` in code
- [ ] Content scripts use isolated world (no DOM prototype extension)
- [ ] Message handlers validate `sender.id` and `sender.tab`
- [ ] Background handlers are allowlisted operations (no generic fetch proxy)
- [ ] Extension storage is per-profile (no cross-profile leakage)

---

## 10. Privacy: Dummy System Profile

In Settings → Privacy Peak → **Dummy System Profile**:

- Fake OS, CPU, GPU, screen resolution, timezone, locale
- Combine with per-profile proxy for IP isolation matching dummy location
- Websites see a completely different machine than your real hardware
- Fingerprint values are internally consistent (Windows profile reports matching UA, WebGL, canvas, fonts, screen)

---

## Security Contact

Report security issues to: security@mybrowser.local

All reports will be acknowledged within 48 hours.
