//! Tab Management Module
//!
//! Chrome/Safari-inspired tab management with:
//! - Tab groups (named, colored, collapsible)
//! - Tab pinning (shrink to favicon, lock to left)
//! - Tab search (open + recently closed)
//! - Mute tabs (per-tab audio muting)
//! - Tab freeze / Memory Saver (suspend inactive tabs)
//! - Vertical tabs (sidebar layout)
//! - Restore closed tabs (session history)
//! - Smooth animations (transform + opacity only, GPU composited, 16ms frame budget)
//! - Per-profile isolation (each profile has its own tab state)

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// A unique tab identifier (u64 for Copy semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

impl TabId {
    pub fn new() -> Self {
        Self(crate::tab_management::ID_GENERATOR.with(|g| {
            let n = g.get();
            g.set(n + 1);
            n
        }))
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// A unique tab group identifier (u64 for Copy semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub u64);

impl GroupId {
    pub fn new() -> Self {
        Self(crate::tab_management::ID_GENERATOR.with(|g| {
            let n = g.get();
            g.set(n + 1);
            n
        }))
    }
}

/// Tab group color (Chrome-style 9 colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupColor {
    Grey,
    Blue,
    Red,
    Yellow,
    Green,
    Pink,
    Purple,
    Cyan,
    Orange,
}

impl GroupColor {
    pub fn as_hex(self) -> &'static str {
        match self {
            GroupColor::Grey => "#686f78",
            GroupColor::Blue => "#4285f4",
            GroupColor::Red => "#ea4335",
            GroupColor::Yellow => "#fbbc05",
            GroupColor::Green => "#34a853",
            GroupColor::Pink => "#e377c2",
            GroupColor::Purple => "#8e44ad",
            GroupColor::Cyan => "#17a2b8",
            GroupColor::Orange => "#ff9800",
        }
    }

    pub fn variants() -> Vec<GroupColor> {
        vec![
            GroupColor::Grey,
            GroupColor::Blue,
            GroupColor::Red,
            GroupColor::Yellow,
            GroupColor::Green,
            GroupColor::Pink,
            GroupColor::Purple,
            GroupColor::Cyan,
            GroupColor::Orange,
        ]
    }
}

/// A tab group — named, colored, collapsible cluster of tabs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabGroup {
    pub id: GroupId,
    pub name: String,
    pub color: GroupColor,
    pub collapsed: bool,
    /// Tab IDs in this group, in order.
    pub tab_ids: Vec<TabId>,
    pub created_at: u64,
}

impl TabGroup {
    pub fn new(name: String, color: GroupColor) -> Self {
        Self {
            id: GroupId::new(),
            name,
            color,
            collapsed: false,
            tab_ids: Vec::new(),
            created_at: now_ms(),
        }
    }

    pub fn contains(&self, tab_id: TabId) -> bool {
        self.tab_ids.contains(&tab_id)
    }
}

/// The lifecycle state of a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabState {
    /// Tab is active and visible.
    Active,
    /// Tab is loaded but not visible.
    Background,
    /// Tab is pinned (shrunk, locked left, no close button).
    Pinned,
    /// Tab is frozen/suspended (JS paused, kept in RAM).
    Frozen,
    /// Tab is discarded (unloaded from RAM, will reload on click).
    Discarded,
    /// Tab is loading.
    Loading,
    /// Tab is muted (audio silenced).
    Muted,
}

/// A single browser tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub favicon_url: Option<String>,
    pub state: TabState,
    pub pinned: bool,
    pub muted: bool,
    pub frozen: bool,
    pub discarded: bool,
    /// Which group this tab belongs to (None = ungrouped).
    pub group_id: Option<GroupId>,
    /// Position within the tab strip (lower = more left).
    pub position: u32,
    /// When the tab was created.
    pub created_at: u64,
    /// When the tab was last accessed.
    pub last_accessed: u64,
    /// Page load progress (0.0 to 1.0).
    pub load_progress: f32,
    /// Whether audio is currently playing.
    pub audio_playing: bool,
    /// Screenshot / thumbnail URL for tab overview.
    pub thumbnail_url: Option<String>,
    /// Site isolation flags.
    pub site_isolated: bool,
}

impl Tab {
    pub fn new(url: String, title: String) -> Self {
        let now = now_ms();
        Self {
            id: TabId::new(),
            url,
            title,
            favicon_url: None,
            state: TabState::Loading,
            pinned: false,
            muted: false,
            frozen: false,
            discarded: false,
            group_id: None,
            position: 0,
            created_at: now,
            last_accessed: now,
            load_progress: 0.0,
            audio_playing: false,
            thumbnail_url: None,
            site_isolated: true,
        }
    }

    pub fn is_closable(&self) -> bool {
        !self.pinned
    }

    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            &self.url
        } else {
            &self.title
        }
    }
}

/// Animation type for tab transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabAnimation {
    /// Tab sliding in from right (new tab opened).
    SlideInRight,
    /// Tab sliding out to right (tab closed).
    SlideOutRight,
    /// Tab fading in.
    FadeIn,
    /// Tab fading out.
    FadeOut,
    /// Tab moving to new position (drag reorder).
    SlideToPosition,
    /// Tab group collapsing/expanding.
    GroupCollapse,
    /// Tab group expanding.
    GroupExpand,
    /// Tab freeze/unfreeze pulse.
    FreezePulse,
}

/// Animation configuration for smooth 60fps transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabAnimationConfig {
    pub duration_ms: u64,
    pub easing: Easing,
    /// Only transform and opacity are animated (GPU composited).
    pub use_gpu_composite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Spring,
}

impl Default for TabAnimationConfig {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            easing: Easing::EaseOut,
            use_gpu_composite: true,
        }
    }
}

/// A running tab animation (runtime only, not serialized).
#[derive(Debug, Clone)]
pub struct RunningAnimation {
    pub animation_type: TabAnimation,
    pub tab_id: Option<TabId>,
    pub group_id: Option<GroupId>,
    pub start_time: u64,
    pub duration_ms: u64,
    pub easing: Easing,
    pub from_transform: (f32, f32), // translateX, translateY
    pub to_transform: (f32, f32),
    pub from_opacity: f32,
    pub to_opacity: f32,
}

impl RunningAnimation {
    pub fn progress(&self) -> f32 {
        let now = now_ms();
        let elapsed = now.saturating_sub(self.start_time);
        if elapsed >= self.duration_ms {
            return 1.0;
        }
        elapsed as f32 / self.duration_ms as f32
    }

    pub fn is_finished(&self) -> bool {
        self.progress() >= 1.0
    }

    /// Compute current transform (translateX, translateY) based on easing.
    pub fn current_transform(&self) -> (f32, f32) {
        let t = self.ease(self.progress());
        (
            self.from_transform.0 + (self.to_transform.0 - self.from_transform.0) * t,
            self.from_transform.1 + (self.to_transform.1 - self.from_transform.1) * t,
        )
    }

    /// Compute current opacity.
    pub fn current_opacity(&self) -> f32 {
        let t = self.ease(self.progress());
        self.from_opacity + (self.to_opacity - self.from_opacity) * t
    }

    fn ease(&self, t: f32) -> f32 {
        match self.easing {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::Spring => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - (-10.0 * t).exp() * (20.0 * t).sin()
                }
            }
        }
    }
}

/// Recently closed tab entry for "Restore Closed Tab" (Ctrl+Shift+T / Cmd+Shift+T).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTabEntry {
    pub tab: Tab,
    pub closed_at: u64,
    pub window_id: Option<String>,
}

impl ClosedTabEntry {
    pub fn new(tab: Tab) -> Self {
        Self {
            tab,
            closed_at: now_ms(),
            window_id: None,
        }
    }
}

/// Tab search result.
#[derive(Debug, Clone)]
pub struct TabSearchResult {
    pub tab_id: TabId,
    pub title: String,
    pub url: String,
    pub group_name: Option<String>,
    pub is_pinned: bool,
    pub score: f32,
}

/// Tab freeze / Memory Saver configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeConfig {
    /// Freeze tabs inactive for more than this many ms.
    pub freeze_after_ms: u64,
    /// Discard (unload) tabs inactive for more than this many ms.
    pub discard_after_ms: u64,
    /// Whether Memory Saver is enabled.
    pub enabled: bool,
    /// Domains that should never be frozen/discarded.
    pub never_freeze_domains: Vec<String>,
}

impl Default for FreezeConfig {
    fn default() -> Self {
        Self {
            freeze_after_ms: 5 * 60 * 1000, // 5 minutes
            discard_after_ms: 30 * 60 * 1000, // 30 minutes
            enabled: true,
            never_freeze_domains: vec![],
        }
    }
}

/// Tab strip position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabPosition {
    Top,
    Side,
}

impl Default for TabPosition {
    fn default() -> Self {
        TabPosition::Top
    }
}

/// Per-profile tab manager state (serializable, animations excluded).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabManagerState {
    /// All tabs keyed by ID.
    pub tabs: HashMap<TabId, Tab>,
    /// Ordered list of tab IDs (position in tab strip).
    pub tab_order: Vec<TabId>,
    /// Currently active tab ID.
    pub active_tab: Option<TabId>,
    /// Tab groups keyed by ID.
    pub groups: HashMap<GroupId, TabGroup>,
    /// Ordered list of group IDs.
    pub group_order: Vec<GroupId>,
    /// Recently closed tabs (for restore).
    pub recently_closed: VecDeque<ClosedTabEntry>,
    /// Maximum recently closed entries.
    pub max_recently_closed: usize,
    /// Freeze / Memory Saver config.
    pub freeze_config: FreezeConfig,
    /// Vertical tabs enabled?
    pub vertical_tabs: bool,
    /// Tab strip position: "top" or "side".
    pub tab_position: TabPosition,
    /// Animation config.
    pub animation_config: TabAnimationConfig,
}

impl Default for TabManagerState {
    fn default() -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab: None,
            groups: HashMap::new(),
            group_order: Vec::new(),
            recently_closed: VecDeque::new(),
            max_recently_closed: 50,
            freeze_config: FreezeConfig::default(),
            vertical_tabs: false,
            tab_position: TabPosition::Top,
            animation_config: TabAnimationConfig::default(),
        }
    }
}

/// ID generator for TabId and GroupId (thread-local counter).
thread_local! {
    static ID_GENERATOR: std::cell::Cell<u64> = std::cell::Cell::new(1);
}

/// Main tab manager — handles all tab operations.
pub struct TabManager {
    state: TabManagerState,
    /// Runtime-only animations (not serialized).
    animations: Vec<RunningAnimation>,
}

impl TabManager {
    /// Create a new tab manager for a profile.
    pub fn new() -> Self {
        Self {
            state: TabManagerState::default(),
            animations: Vec::new(),
        }
    }

    /// Create a new tab manager with vertical tabs enabled.
    pub fn with_vertical_tabs() -> Self {
        let mut state = TabManagerState::default();
        state.vertical_tabs = true;
        state.tab_position = TabPosition::Side;
        Self {
            state,
            animations: Vec::new(),
        }
    }

    // ── Tab Creation ──
    /// Open a new tab and make it active.
    pub fn open_tab(&mut self, url: String, title: String) -> TabId {
        let mut tab = Tab::new(url.clone(), title);
        let id = tab.id;

        // If there's an active tab, insert after it; otherwise append.
        let insert_pos = match self.state.active_tab {
            Some(active_id) => {
                let idx = self
                    .state
                    .tab_order
                    .iter()
                    .position(|t| *t == active_id)
                    .unwrap_or(0);
                idx + 1
            }
            None => self.state.tab_order.len(),
        };

        tab.position = insert_pos as u32;
        self.state.tab_order.insert(insert_pos, id);
        self.state.tabs.insert(id, tab);
        self.state.active_tab = Some(id);

        // Add slide-in animation.
        self.add_animation(RunningAnimation {
            animation_type: TabAnimation::SlideInRight,
            tab_id: Some(id),
            group_id: None,
            start_time: now_ms(),
            duration_ms: self.state.animation_config.duration_ms,
            easing: self.state.animation_config.easing,
            from_transform: (20.0, 0.0), // slide in from right
            to_transform: (0.0, 0.0),
            from_opacity: 0.0,
            to_opacity: 1.0,
        });

        id
    }

    /// Open a new tab in a specific group.
    pub fn open_tab_in_group(&mut self, url: String, title: String, group_id: GroupId) -> TabId {
        let tab_id = self.open_tab(url, title);
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            tab.group_id = Some(group_id);
        }
        if let Some(group) = self.state.groups.get_mut(&group_id) {
            if !group.tab_ids.contains(&tab_id) {
                group.tab_ids.push(tab_id);
            }
        }
        tab_id
    }

    /// Duplicate a tab.
    pub fn duplicate_tab(&mut self, tab_id: TabId) -> Option<TabId> {
        let tab = self.state.tabs.get(&tab_id)?.clone();
        let new_id = self.open_tab(tab.url.clone(), tab.title.clone());
        if let Some(new_tab) = self.state.tabs.get_mut(&new_id) {
            new_tab.favicon_url = tab.favicon_url;
            new_tab.group_id = tab.group_id;
            new_tab.muted = tab.muted;
            new_tab.site_isolated = tab.site_isolated;
        }
        if let Some(gid) = tab.group_id {
            if let Some(group) = self.state.groups.get_mut(&gid) {
                if !group.tab_ids.contains(&new_id) {
                    group.tab_ids.push(new_id);
                }
            }
        }
        Some(new_id)
    }

    // ── Tab Closing ──
    /// Close a tab. If it's the active tab, activate the next/previous one.
    pub fn close_tab(&mut self, tab_id: TabId) -> Option<TabId> {
        // Remove from tab order.
        let idx = self.state.tab_order.iter().position(|t| *t == tab_id)?;
        self.state.tab_order.remove(idx);

        // Remove from group if present.
        if let Some(tab) = self.state.tabs.get(&tab_id) {
            if let Some(gid) = tab.group_id {
                if let Some(group) = self.state.groups.get_mut(&gid) {
                    group.tab_ids.retain(|t| *t != tab_id);
                }
            }
        }

        // Save to recently closed before removing.
        if let Some(tab) = self.state.tabs.get(&tab_id).cloned() {
            self.add_recently_closed(ClosedTabEntry::new(tab));
        }

        // Remove the tab.
        self.state.tabs.remove(&tab_id);

        // Determine next active tab.
        let next_active = if self.state.tab_order.is_empty() {
            None
        } else {
            let next_idx = if idx >= self.state.tab_order.len() {
                self.state.tab_order.len() - 1
            } else {
                idx
            };
            Some(self.state.tab_order[next_idx])
        };

        // Close animation.
        if let Some(next_id) = next_active {
            self.add_animation(RunningAnimation {
                animation_type: TabAnimation::SlideOutRight,
                tab_id: Some(tab_id),
                group_id: None,
                start_time: now_ms(),
                duration_ms: self.state.animation_config.duration_ms,
                easing: self.state.animation_config.easing,
                from_transform: (0.0, 0.0),
                to_transform: (20.0, 0.0),
                from_opacity: 1.0,
                to_opacity: 0.0,
            });
        }

        self.state.active_tab = next_active;
        next_active
    }

    /// Close all tabs except the given one.
    pub fn close_other_tabs(&mut self, keep_tab_id: TabId) -> Vec<TabId> {
        let mut closed = Vec::new();
        let to_close: Vec<TabId> = self
            .state
            .tab_order
            .iter()
            .filter(|t| **t != keep_tab_id)
            .map(|t| *t)
            .collect();
        for id in to_close {
            if self.close_tab(id).is_some() {
                closed.push(id);
            }
        }
        // Make sure the kept tab is active.
        self.state.active_tab = Some(keep_tab_id);
        closed
    }

    /// Close tabs to the right of the given tab.
    pub fn close_tabs_to_right(&mut self, tab_id: TabId) -> Vec<TabId> {
        let mut closed = Vec::new();
        let idx = match self.state.tab_order.iter().position(|t| *t == tab_id) {
            Some(i) => i,
            None => return closed,
        };
        let to_close: Vec<TabId> = self.state.tab_order[idx + 1..].to_vec();
        for id in to_close {
            if self.close_tab(id).is_some() {
                closed.push(id);
            }
        }
        closed
    }

    // ── Tab Switching ──
    /// Switch to a different tab by ID.
    pub fn switch_to_tab(&mut self, tab_id: TabId) -> bool {
        if self.state.tabs.contains_key(&tab_id) {
            self.state.active_tab = Some(tab_id);
            if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
                tab.last_accessed = now_ms();
                tab.state = TabState::Active;
            }
            // Unfreeze if frozen.
            if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
                tab.frozen = false;
                tab.discarded = false;
                tab.state = TabState::Active;
            }
            true
        } else {
            false
        }
    }

    /// Switch to the next tab (Ctrl+Tab).
    pub fn next_tab(&mut self) -> Option<TabId> {
        let active = self.state.active_tab?;
        let idx = self.state.tab_order.iter().position(|t| *t == active)?;
        let next_idx = (idx + 1) % self.state.tab_order.len();
        let next_id = self.state.tab_order[next_idx];
        self.switch_to_tab(next_id);
        Some(next_id)
    }

    /// Switch to the previous tab (Ctrl+Shift+Tab).
    pub fn prev_tab(&mut self) -> Option<TabId> {
        let active = self.state.active_tab?;
        let idx = self.state.tab_order.iter().position(|t| *t == active)?;
        let prev_idx = if idx == 0 {
            self.state.tab_order.len() - 1
        } else {
            idx - 1
        };
        let prev_id = self.state.tab_order[prev_idx];
        self.switch_to_tab(prev_id);
        Some(prev_id)
    }

    // ── Tab Pinning ──
    /// Pin a tab (shrinks to favicon, locks left, no close button).
    pub fn pin_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            if !tab.pinned {
                tab.pinned = true;
                tab.state = TabState::Pinned;
                // Move to front of tab order.
                if let Some(idx) = self.state.tab_order.iter().position(|t| *t == tab_id) {
                    self.state.tab_order.remove(idx);
                    self.state.tab_order.insert(0, tab_id);
                    tab.position = 0;
                }
                // Re-number positions.
                self.recompute_positions();
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Unpin a tab.
    pub fn unpin_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            if tab.pinned {
                tab.pinned = false;
                tab.state = TabState::Background;
                self.recompute_positions();
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    // ── Tab Muting ──
    /// Mute a tab.
    pub fn mute_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            tab.muted = true;
            tab.audio_playing = false;
            true
        } else {
            false
        }
    }

    /// Unmute a tab.
    pub fn unmute_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            tab.muted = false;
            true
        } else {
            false
        }
    }

    /// Toggle mute on a tab.
    pub fn toggle_mute_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get(&tab_id) {
            if tab.muted {
                self.unmute_tab(tab_id)
            } else {
                self.mute_tab(tab_id)
            }
        } else {
            false
        }
    }

    // ── Tab Freeze / Memory Saver ──
    /// Freeze a tab (pause JS, keep in RAM).
    pub fn freeze_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            if !tab.pinned && !tab.frozen {
                tab.frozen = true;
                tab.state = TabState::Frozen;
                self.add_animation(RunningAnimation {
                    animation_type: TabAnimation::FreezePulse,
                    tab_id: Some(tab_id),
                    group_id: None,
                    start_time: now_ms(),
                    duration_ms: 1000,
                    easing: Easing::EaseInOut,
                    from_transform: (0.0, 0.0),
                    to_transform: (0.0, 0.0),
                    from_opacity: 1.0,
                    to_opacity: 0.7,
                });
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Unfreeze a tab.
    pub fn unfreeze_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            if tab.frozen {
                tab.frozen = false;
                tab.state = TabState::Background;
                self.add_animation(RunningAnimation {
                    animation_type: TabAnimation::FreezePulse,
                    tab_id: Some(tab_id),
                    group_id: None,
                    start_time: now_ms(),
                    duration_ms: 500,
                    easing: Easing::EaseOut,
                    from_transform: (0.0, 0.0),
                    to_transform: (0.0, 0.0),
                    from_opacity: 0.7,
                    to_opacity: 1.0,
                });
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Discard a tab (unload from RAM).
    pub fn discard_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            if !tab.pinned && !tab.discarded {
                tab.discarded = true;
                tab.frozen = true;
                tab.state = TabState::Discarded;
                tab.load_progress = 0.0;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Run automatic freeze/discard based on inactivity (Memory Saver).
    pub fn run_memory_saver(&mut self) {
        if !self.state.freeze_config.enabled {
            return;
        }

        let now = now_ms();
        let active_id = self.state.active_tab;

        for tab_id in self.state.tab_order.clone() {
            // Skip active and pinned tabs.
            if Some(tab_id) == active_id {
                continue;
            }

            let tab = match self.state.tabs.get(&tab_id) {
                Some(t) => t.clone(),
                None => continue,
            };

            // Check never-freeze domains.
            let domain = extract_domain(&tab.url);
            if self.state.freeze_config.never_freeze_domains.contains(&domain) {
                continue;
            }

            let inactive_ms = now.saturating_sub(tab.last_accessed);

            if inactive_ms >= self.state.freeze_config.discard_after_ms && !tab.discarded {
                self.discard_tab(tab_id);
            } else if inactive_ms >= self.state.freeze_config.freeze_after_ms && !tab.frozen && !tab.discarded {
                self.freeze_tab(tab_id);
            }
        }
    }

    // ── Tab Groups ──
    /// Create a new tab group.
    pub fn create_group(&mut self, name: String, color: GroupColor) -> GroupId {
        let group = TabGroup::new(name, color);
        let id = group.id;
        self.state.groups.insert(id, group);
        self.state.group_order.push(id);
        id
    }

    /// Delete a group and move its tabs to ungrouped.
    pub fn delete_group(&mut self, group_id: GroupId) -> bool {
        if let Some(group) = self.state.groups.remove(&group_id) {
            self.state.group_order.retain(|g| *g != group_id);
            for tab_id in group.tab_ids {
                if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
                    tab.group_id = None;
                }
            }
            true
        } else {
            false
        }
    }

    /// Add a tab to a group.
    pub fn add_tab_to_group(&mut self, tab_id: TabId, group_id: GroupId) -> bool {
        if self.state.tabs.contains_key(&tab_id) && self.state.groups.contains_key(&group_id) {
            if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
                // Remove from old group if any.
                if let Some(old_gid) = tab.group_id {
                    if let Some(old_group) = self.state.groups.get_mut(&old_gid) {
                        old_group.tab_ids.retain(|t| *t != tab_id);
                    }
                }
                tab.group_id = Some(group_id);
            }
            if let Some(group) = self.state.groups.get_mut(&group_id) {
                if !group.tab_ids.contains(&tab_id) {
                    group.tab_ids.push(tab_id);
                }
            }
            true
        } else {
            false
        }
    }

    /// Remove a tab from its group (ungroup).
    pub fn remove_tab_from_group(&mut self, tab_id: TabId) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            if let Some(gid) = tab.group_id {
                tab.group_id = None;
                if let Some(group) = self.state.groups.get_mut(&gid) {
                    group.tab_ids.retain(|t| *t != tab_id);
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Toggle group collapsed state.
    pub fn toggle_group_collapse(&mut self, group_id: GroupId) -> bool {
        let collapsed = if let Some(group) = self.state.groups.get_mut(&group_id) {
            group.collapsed = !group.collapsed;
            group.collapsed
        } else {
            return false;
        };
        self.add_animation(RunningAnimation {
            animation_type: if collapsed {
                TabAnimation::GroupCollapse
            } else {
                TabAnimation::GroupExpand
            },
            tab_id: None,
            group_id: Some(group_id),
            start_time: now_ms(),
            duration_ms: self.state.animation_config.duration_ms,
            easing: self.state.animation_config.easing,
            from_transform: (0.0, 0.0),
            to_transform: (0.0, 0.0),
            from_opacity: 1.0,
            to_opacity: if collapsed { 0.0 } else { 1.0 },
        });
        true
    }

    /// Rename a group.
    pub fn rename_group(&mut self, group_id: GroupId, new_name: String) -> bool {
        if let Some(group) = self.state.groups.get_mut(&group_id) {
            group.name = new_name;
            true
        } else {
            false
        }
    }

    /// Set group color.
    pub fn set_group_color(&mut self, group_id: GroupId, color: GroupColor) -> bool {
        if let Some(group) = self.state.groups.get_mut(&group_id) {
            group.color = color;
            true
        } else {
            false
        }
    }

    // ── Tab Search ──
    /// Search tabs by title or URL. Returns ranked results.
    pub fn search_tabs(&self, query: &str) -> Vec<TabSearchResult> {
        let query = query.to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<TabSearchResult> = Vec::new();

        for tab_id in &self.state.tab_order {
            let tab = match self.state.tabs.get(tab_id) {
                Some(t) => t,
                None => continue,
            };

            let title_lower = tab.title.to_lowercase();
            let url_lower = tab.url.to_lowercase();

            let score = if title_lower.contains(&query) {
                10.0
            } else if url_lower.contains(&query) {
                5.0
            } else {
                continue;
            };

            let group_name = tab.group_id.as_ref().and_then(|gid| {
                self.state.groups.get(gid).map(|g| g.name.clone())
            });

            results.push(TabSearchResult {
                tab_id: *tab_id,
                title: tab.title.clone(),
                url: tab.url.clone(),
                group_name,
                is_pinned: tab.pinned,
                score,
            });
        }

        // Also search recently closed tabs.
        for entry in &self.state.recently_closed {
            let title_lower = entry.tab.title.to_lowercase();
            let url_lower = entry.tab.url.to_lowercase();
            if title_lower.contains(&query) || url_lower.contains(&query) {
                results.push(TabSearchResult {
                    tab_id: entry.tab.id,
                    title: entry.tab.title.clone(),
                    url: entry.tab.url.clone(),
                    group_name: entry.tab.group_id.as_ref().and_then(|gid| {
                        self.state.groups.get(gid).map(|g| g.name.clone())
                    }),
                    is_pinned: entry.tab.pinned,
                    score: 2.0, // lower priority for closed tabs
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    // ── Restore Closed Tab ──
    /// Restore the most recently closed tab (Ctrl+Shift+T / Cmd+Shift+T).
    pub fn restore_closed_tab(&mut self) -> Option<TabId> {
        let entry = self.state.recently_closed.pop_front()?;
        let tab = entry.tab;

        // Re-open the tab.
        let new_id = self.open_tab(tab.url.clone(), tab.title.clone());
        if let Some(new_tab) = self.state.tabs.get_mut(&new_id) {
            new_tab.favicon_url = tab.favicon_url;
            new_tab.group_id = tab.group_id;
            new_tab.muted = tab.muted;
            new_tab.site_isolated = tab.site_isolated;
        }
        if let Some(gid) = tab.group_id {
            if let Some(group) = self.state.groups.get_mut(&gid) {
                if !group.tab_ids.contains(&new_id) {
                    group.tab_ids.push(new_id);
                }
            }
        }
        Some(new_id)
    }

    /// Get list of recently closed tabs (for UI display).
    pub fn recently_closed_list(&self) -> Vec<&ClosedTabEntry> {
        self.state.recently_closed.iter().collect()
    }

    // ── Tab Reordering (drag) ──
    /// Move a tab to a new position in the tab order.
    pub fn move_tab_to_position(&mut self, tab_id: TabId, new_position: usize) -> bool {
        if !self.state.tabs.contains_key(&tab_id) {
            return false;
        }
        let old_idx = match self.state.tab_order.iter().position(|t| *t == tab_id) {
            Some(i) => i,
            None => return false,
        };
        let new_position = new_position.min(self.state.tab_order.len().saturating_sub(1));
        self.state.tab_order.remove(old_idx);
        self.state.tab_order.insert(new_position, tab_id);
        self.recompute_positions();

        self.add_animation(RunningAnimation {
            animation_type: TabAnimation::SlideToPosition,
            tab_id: Some(tab_id),
            group_id: None,
            start_time: now_ms(),
            duration_ms: 200,
            easing: Easing::EaseOut,
            from_transform: (0.0, 0.0),
            to_transform: (0.0, 0.0),
            from_opacity: 1.0,
            to_opacity: 1.0,
        });
        true
    }

    /// Move a tab between groups.
    pub fn move_tab_to_group(&mut self, tab_id: TabId, new_group_id: Option<GroupId>) -> bool {
        if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
            // Remove from old group.
            if let Some(old_gid) = tab.group_id {
                if let Some(old_group) = self.state.groups.get_mut(&old_gid) {
                    old_group.tab_ids.retain(|t| *t != tab_id);
                }
            }
            tab.group_id = new_group_id;
            // Add to new group.
            if let Some(gid) = new_group_id {
                if let Some(group) = self.state.groups.get_mut(&gid) {
                    if !group.tab_ids.contains(&tab_id) {
                        group.tab_ids.push(tab_id);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    // ── Vertical Tabs ──
    pub fn set_vertical_tabs(&mut self, enabled: bool) {
        self.state.vertical_tabs = enabled;
        self.state.tab_position = if enabled {
            TabPosition::Side
        } else {
            TabPosition::Top
        };
    }

    pub fn is_vertical_tabs(&self) -> bool {
        self.state.vertical_tabs
    }

    // ── Animation updates ──
    /// Add a running animation.
    fn add_animation(&mut self, anim: RunningAnimation) {
        self.animations.push(anim);
        // Clean up finished animations.
        self.animations.retain(|a| !a.is_finished());
        // Limit animation count.
        if self.animations.len() > 20 {
            self.animations.drain(0..self.animations.len() - 20);
        }
    }

    /// Update animations (call each frame). Returns true if any animation is still running.
    pub fn update_animations(&mut self) -> bool {
        let now = now_ms();
        self.animations.retain(|a| {
            let elapsed = now.saturating_sub(a.start_time);
            elapsed < a.duration_ms
        });
        !self.animations.is_empty()
    }

    /// Get all running animations.
    pub fn animations(&self) -> &[RunningAnimation] {
        &self.animations
    }

    // ── Query methods ──
    pub fn tab_count(&self) -> usize {
        self.state.tabs.len()
    }

    pub fn active_tab(&self) -> Option<TabId> {
        self.state.active_tab
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<&Tab> {
        self.state.tabs.get(&tab_id)
    }

    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut Tab> {
        self.state.tabs.get_mut(&tab_id)
    }

    pub fn tab_order(&self) -> &[TabId] {
        &self.state.tab_order
    }

    pub fn groups(&self) -> &HashMap<GroupId, TabGroup> {
        &self.state.groups
    }

    pub fn group_order(&self) -> &[GroupId] {
        &self.state.group_order
    }

    pub fn is_vertical(&self) -> bool {
        self.state.vertical_tabs
    }

    pub fn tab_position(&self) -> TabPosition {
        self.state.tab_position
    }

    pub fn animation_config(&self) -> &TabAnimationConfig {
        &self.state.animation_config
    }

    pub fn set_animation_config(&mut self, config: TabAnimationConfig) {
        self.state.animation_config = config;
    }

    pub fn freeze_config(&self) -> &FreezeConfig {
        &self.state.freeze_config
    }

    pub fn set_freeze_config(&mut self, config: FreezeConfig) {
        self.state.freeze_config = config;
    }

    // ── Private helpers ──
    fn recompute_positions(&mut self) {
        for (idx, tab_id) in self.state.tab_order.iter().enumerate() {
            if let Some(tab) = self.state.tabs.get_mut(tab_id) {
                tab.position = idx as u32;
            }
        }
    }

    fn add_recently_closed(&mut self, entry: ClosedTabEntry) {
        self.state.recently_closed.push_front(entry);
        while self.state.recently_closed.len() > self.state.max_recently_closed {
            self.state.recently_closed.pop_back();
        }
    }
}

fn extract_domain(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped.split('/').next().unwrap_or(url).to_string()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub mod ui;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_close_tab() {
        let mut mgr = TabManager::new();
        let id = mgr.open_tab("https://example.com".to_string(), "Example".to_string());
        assert_eq!(mgr.active_tab(), Some(id));
        assert_eq!(mgr.tab_count(), 1);

        let next = mgr.close_tab(id);
        assert_eq!(next, None);
        assert_eq!(mgr.tab_count(), 0);
    }

    #[test]
    fn tab_groups() {
        let mut mgr = TabManager::new();
        let gid = mgr.create_group("Work".to_string(), GroupColor::Blue);
        let id1 = mgr.open_tab("https://work1.com".to_string(), "Work 1".to_string());
        let id2 = mgr.open_tab("https://work2.com".to_string(), "Work 2".to_string());

        mgr.add_tab_to_group(id1, gid);
        mgr.add_tab_to_group(id2, gid);

        let group = mgr.groups().get(&gid).unwrap();
        assert_eq!(group.tab_ids.len(), 2);
        assert_eq!(group.name, "Work");

        mgr.toggle_group_collapse(gid);
        let group = mgr.groups().get(&gid).unwrap();
        assert!(group.collapsed);
    }

    #[test]
    fn pin_tab() {
        let mut mgr = TabManager::new();
        let id = mgr.open_tab("https://example.com".to_string(), "Example".to_string());
        assert!(mgr.pin_tab(id));
        let tab = mgr.get_tab(id).unwrap();
        assert!(tab.pinned);
        assert!(!tab.is_closable());
        assert!(mgr.unpin_tab(id));
    }

    #[test]
    fn mute_tab() {
        let mut mgr = TabManager::new();
        let id = mgr.open_tab("https://example.com".to_string(), "Example".to_string());
        assert!(mgr.mute_tab(id));
        assert!(mgr.get_tab(id).unwrap().muted);
        assert!(mgr.unmute_tab(id));
        assert!(!mgr.get_tab(id).unwrap().muted);
    }

    #[test]
    fn freeze_and_unfreeze() {
        let mut mgr = TabManager::new();
        let id1 = mgr.open_tab("https://a.com".to_string(), "A".to_string());
        let id2 = mgr.open_tab("https://b.com".to_string(), "B".to_string());
        mgr.switch_to_tab(id1); // id1 is active

        // Freeze id2 (inactive).
        assert!(mgr.freeze_tab(id2));
        assert!(mgr.get_tab(id2).unwrap().frozen);

        // Unfreeze.
        assert!(mgr.unfreeze_tab(id2));
        assert!(!mgr.get_tab(id2).unwrap().frozen);
    }

    #[test]
    fn search_tabs() {
        let mut mgr = TabManager::new();
        mgr.open_tab("https://github.com/myuser/myrepo".to_string(), "My Repo - GitHub".to_string());
        mgr.open_tab("https://example.com".to_string(), "Example Domain".to_string());

        let results = mgr.search_tabs("github");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "My Repo - GitHub");

        let results = mgr.search_tabs("example");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Domain");

        let results = mgr.search_tabs("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn restore_closed_tab() {
        let mut mgr = TabManager::new();
        let id = mgr.open_tab("https://example.com".to_string(), "Example".to_string());
        mgr.close_tab(id);

        let restored = mgr.restore_closed_tab();
        assert!(restored.is_some());
        assert_eq!(mgr.tab_count(), 1);
    }

    #[test]
    fn vertical_tabs() {
        let mut mgr = TabManager::new();
        assert!(!mgr.is_vertical());
        mgr.set_vertical_tabs(true);
        assert!(mgr.is_vertical());
        assert_eq!(mgr.tab_position(), TabPosition::Side);
    }

    #[test]
    fn close_other_tabs() {
        let mut mgr = TabManager::new();
        let id1 = mgr.open_tab("https://a.com".to_string(), "A".to_string());
        let id2 = mgr.open_tab("https://b.com".to_string(), "B".to_string());
        let id3 = mgr.open_tab("https://c.com".to_string(), "C".to_string());

        mgr.close_other_tabs(id2);
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_tab(), Some(id2));
    }

    #[test]
    fn animation_progress() {
        let anim = RunningAnimation {
            animation_type: TabAnimation::SlideInRight,
            tab_id: Some(TabId::new()),
            group_id: None,
            start_time: now_ms(),
            duration_ms: 300,
            easing: Easing::EaseOut,
            from_transform: (20.0, 0.0),
            to_transform: (0.0, 0.0),
            from_opacity: 0.0,
            to_opacity: 1.0,
        };
        // Should be at ~0 progress at start.
        assert!((anim.progress() - 0.0).abs() < 0.1);
        assert!(!anim.is_finished());
    }
}
