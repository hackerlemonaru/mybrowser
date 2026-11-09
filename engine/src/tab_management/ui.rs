//! Tab Management UI — egui-based tab bar with groups, pinning, mute,
//! freeze, search, and vertical tabs. All animations use GPU-composited
//! transform + opacity only for 60fps smoothness.

use crate::tab_management::{
    TabAnimation, TabAnimationConfig, TabId, TabManager, TabPosition, GroupColor, GroupId,
    RunningAnimation, Easing,
};
use egui::{
    Align2, Color32, Frame, Rounding, Stroke, Vec2, Align, Layout, TextStyle, RichText,
};
use std::collections::HashMap;

/// Result of tab UI interaction.
#[derive(Debug, Clone)]
pub enum TabUIAction {
    None,
    OpenTab(String),                          // URL
    CloseTab(TabId),
    SwitchTab(TabId),
    PinTab(TabId),
    UnpinTab(TabId),
    MuteTab(TabId),
    UnmuteTab(TabId),
    FreezeTab(TabId),
    UnfreezeTab(TabId),
    DiscardTab(TabId),
    DuplicateTab(TabId),
    CreateGroup(String, GroupColor),
    DeleteGroup(GroupId),
    AddTabToGroup(TabId, GroupId),
    RemoveTabFromGroup(TabId),
    ToggleGroupCollapse(GroupId),
    RenameGroup(GroupId, String),
    SetGroupColor(GroupId, GroupColor),
    RestoreClosedTab,
    SearchTabs(String),
    SetVerticalTabs(bool),
    SetAnimationConfig(TabAnimationConfig),
    MoveTab(TabId, usize), // tab_id, new_position
    CloseOtherTabs(TabId),
    CloseTabsToRight(TabId),
}

/// Tab UI widget — renders the tab bar and handles interactions.
pub struct TabUI {
    /// Whether the search overlay is open.
    search_open: bool,
    search_query: String,
    /// Whether the recently-closed overlay is open.
    recently_closed_open: bool,
    /// Whether the group creation dialog is open.
    group_dialog_open: bool,
    group_dialog_name: String,
    group_dialog_color: GroupColor,
    /// Hovered tab ID for showing close button.
    hovered_tab: Option<TabId>,
    /// Tab being dragged (for reordering).
    drag_source: Option<TabId>,
    /// Last frame time.
    last_frame: Option<std::time::Instant>,
}

impl Default for TabUI {
    fn default() -> Self {
        Self {
            search_open: false,
            search_query: String::new(),
            recently_closed_open: false,
            group_dialog_open: false,
            group_dialog_name: String::new(),
            group_dialog_color: GroupColor::Blue,
            hovered_tab: None,
            drag_source: None,
            last_frame: None,
        }
    }
}

impl TabUI {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the full tab UI (tab bar + overlays). Returns any actions to take.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        mgr: &mut TabManager,
    ) -> TabUIAction {
        let mut action = TabUIAction::None;

        // Update animations each frame.
        self.last_frame = Some(std::time::Instant::now());
        let _ = mgr.update_animations();

        let vertical = mgr.is_vertical();

        // ── Top tab bar (horizontal) or sidebar (vertical) ──
        if vertical {
            egui::SidePanel::left("vertical_tabs")
                .default_width(280.0)
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    self.render_vertical_sidebar(ui, mgr, &mut action);
                });
        } else {
            egui::TopBottomPanel::top("tab_bar")
                .min_height(40.0)
                .show(ui.ctx(), |ui| {
                    self.render_horizontal_tabbar(ui, mgr, &mut action);
                });
        }

        // ── Search overlay ──
        if self.search_open {
            self.render_search_overlay(ui.ctx(), mgr, &mut action);
        }

        // ── Recently closed overlay ──
        if self.recently_closed_open {
            self.render_recently_closed_overlay(ui.ctx(), mgr, &mut action);
        }

        // ── Group creation dialog ──
        if self.group_dialog_open {
            self.render_group_dialog(ui.ctx(), mgr, &mut action);
        }

        action
    }

    // ── Horizontal tab bar ──
    fn render_horizontal_tabbar(
        &mut self,
        ui: &mut egui::Ui,
        mgr: &mut TabManager,
        action: &mut TabUIAction,
    ) {
        ui.horizontal(|ui| {
            ui.spacing();

            // Render groups first (as collapsible headers in the tab bar).
            let group_order: Vec<crate::tab_management::GroupId> = mgr.group_order().to_vec();
            for gid in group_order {
                let group = match mgr.groups().get(&gid) {
                    Some(g) => g.clone(),
                    None => continue,
                };

                if group.tab_ids.is_empty() {
                    continue; // skip empty groups
                }

                let collapsed = group.collapsed;
                let color = parse_hex_color(group.color.as_hex());

                let header_text = if collapsed {
                    format!("▶ {} ({})", group.name, group.tab_ids.len())
                } else {
                    format!("▼ {} ({})", group.name, group.tab_ids.len())
                };

                if ui.button(RichText::new(&header_text).color(color).strong()).clicked() {
                    *action = TabUIAction::ToggleGroupCollapse(gid);
                }

                if !collapsed {
                    self.render_tab_list_in_group(ui, mgr, &group, action);
                }
            }

            // Render ungrouped tabs.
            let ungrouped: Vec<TabId> = mgr
                .tab_order()
                .iter()
                .filter(|tid| {
                    mgr.get_tab(**tid)
                        .map_or(false, |t| t.group_id.is_none())
                })
                .copied()
                .collect();

            if !ungrouped.is_empty() {
                self.render_tab_list(ui, mgr, &ungrouped, None, action);
            }

            // New tab button.
            ui.separator();
            if ui.button("+ New Tab").clicked() {
                *action = TabUIAction::OpenTab("https://example.com".to_string());
            }

            // Keyboard shortcuts hint.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(
                        "Ctrl+Shift+A: Search  │  Ctrl+Shift+T: Restore Closed  │  Ctrl+Tab: Next",
                    )
                    .small()
                    .color(Color32::from_gray(150)),
                );
            });
        });
    }

    // ── Vertical tab sidebar ──
    fn render_vertical_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        mgr: &mut TabManager,
        action: &mut TabUIAction,
    ) {
        ui.vertical(|ui| {
            ui.heading("MyBrowser");
            ui.separator();

            // New tab button at top of sidebar.
            if ui.button("+ New Tab").clicked() {
                *action = TabUIAction::OpenTab("https://example.com".to_string());
            }

            ui.separator();

            // Groups.
            let group_order: Vec<crate::tab_management::GroupId> = mgr.group_order().to_vec();
            for gid in group_order {
                let group = match mgr.groups().get(&gid) {
                    Some(g) => g.clone(),
                    None => continue,
                };

                if group.tab_ids.is_empty() {
                    continue;
                }

                let collapsed = group.collapsed;
                let color = parse_hex_color(group.color.as_hex());

                let header_text = if collapsed {
                    format!("▶ {}", group.name)
                } else {
                    format!("▼ {} ({})", group.name, group.tab_ids.len())
                };

                if ui.button(RichText::new(&header_text).color(color).strong()).clicked() {
                    *action = TabUIAction::ToggleGroupCollapse(gid);
                }

                if !collapsed {
                    self.render_tab_list_in_group(ui, mgr, &group, action);
                }
            }

            // Ungrouped tabs.
            let ungrouped: Vec<TabId> = mgr
                .tab_order()
                .iter()
                .filter(|tid| {
                    mgr.get_tab(**tid)
                        .map_or(false, |t| t.group_id.is_none())
                })
                .copied()
                .collect();

            if !ungrouped.is_empty() {
                ui.label(
                    RichText::new("Ungrouped")
                        .small()
                        .color(Color32::from_gray(150)),
                );
                self.render_tab_list(ui, mgr, &ungrouped, None, action);
            }

            ui.separator();

            // Controls.
            ui.vertical_centered(|ui| {
                if ui.button("Restore Closed Tab").clicked() {
                    *action = TabUIAction::RestoreClosedTab;
                }
                if ui.button("Search Tabs").clicked() {
                    self.search_open = true;
                }
                let vert_label = if mgr.is_vertical() {
                    "Horizontal Tabs"
                } else {
                    "Vertical Tabs"
                };
                if ui.button(vert_label).clicked() {
                    *action = TabUIAction::SetVerticalTabs(!mgr.is_vertical());
                }
            });
        });
    }

    // ── Render a list of tabs ──
    fn render_tab_list(
        &mut self,
        ui: &mut egui::Ui,
        mgr: &mut TabManager,
        tab_ids: &[TabId],
        _group_id: Option<crate::tab_management::GroupId>,
        action: &mut TabUIAction,
    ) {
        for tid in tab_ids {
            let tab = match mgr.get_tab(*tid) {
                Some(t) => t.clone(),
                None => continue,
            };
            self.render_tab_card(ui, mgr, &tab, action);
        }
    }

    fn render_tab_list_in_group(
        &mut self,
        ui: &mut egui::Ui,
        mgr: &mut TabManager,
        group: &crate::tab_management::TabGroup,
        action: &mut TabUIAction,
    ) {
        for tid in &group.tab_ids {
            let tab = match mgr.get_tab(*tid) {
                Some(t) => t.clone(),
                None => continue,
            };
            self.render_tab_card(ui, mgr, &tab, action);
        }
    }

    // ── Individual tab card ──
    fn render_tab_card(
        &mut self,
        ui: &mut egui::Ui,
        mgr: &mut TabManager,
        tab: &crate::tab_management::Tab,
        action: &mut TabUIAction,
    ) {
        let is_active = mgr.active_tab() == Some(tab.id);
        let theme_color = if let Some(gid) = tab.group_id {
            mgr.groups()
                .get(&gid)
                .map_or(Color32::from_rgb(66, 133, 244), |g| {
                    parse_hex_color(g.color.as_hex())
                })
        } else {
            Color32::from_gray(60)
        };

        let is_hovered = self.hovered_tab == Some(tab.id);

        Frame::none()
            .fill(if is_active {
                theme_color.linear_multiply(0.2)
            } else if is_hovered {
                Color32::from_gray(45)
            } else {
                Color32::from_gray(30)
            })
            .rounding(Rounding::same(6.0))
            .stroke(Stroke::new(
                1.0,
                if is_active {
                    theme_color
                } else {
                    Color32::from_gray(50)
                },
            ))
            .show(ui, |ui| {
                ui.set_min_height(32.0);
                ui.horizontal(|ui| {
                    // Pinned indicator.
                    if tab.pinned {
                        ui.label(RichText::new("📌").small());
                    }

                    // Favicon placeholder.
                    ui.painter().circle_filled(ui.cursor().center(), 8.0, Color32::from_gray(100));
                    ui.allocate_space(Vec2::new(16.0, 0.0));

                    // Mute icon.
                    if tab.muted {
                        ui.label(RichText::new("🔇").small());
                    }

                    // Frozen indicator.
                    if tab.frozen {
                        ui.label(RichText::new("❄️").small());
                    }

                    // Discarded indicator.
                    if tab.discarded {
                        ui.label(RichText::new("💤").small());
                    }

                    // Title.
                    let title = tab.display_title();
                    let display_title = if title.len() > 40 {
                        format!("{}...", &title[..37])
                    } else {
                        title.to_string()
                    };

                    if ui.selectable_label(is_active, RichText::new(&display_title)).clicked() {
                        *action = TabUIAction::SwitchTab(tab.id);
                    }

                    // Right-aligned controls.
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Close button (not for pinned tabs).
                        if !tab.pinned {
                            if ui.button("✕").clicked() {
                                *action = TabUIAction::CloseTab(tab.id);
                            }
                        }

                        // Freeze/unfreeze.
                        if tab.frozen {
                            if ui.button("❄️ Unfreeze").clicked() {
                                *action = TabUIAction::UnfreezeTab(tab.id);
                            }
                        } else if !tab.pinned {
                            if ui.button("❄️ Freeze").clicked() {
                                *action = TabUIAction::FreezeTab(tab.id);
                            }
                        }

                        // Mute/unmute.
                        if tab.muted {
                            if ui.button("🔊 Unmute").clicked() {
                                *action = TabUIAction::UnmuteTab(tab.id);
                            }
                        } else {
                            if ui.button("🔇 Mute").clicked() {
                                *action = TabUIAction::MuteTab(tab.id);
                            }
                        }

                        // Pin/unpin.
                        if tab.pinned {
                            if ui.button("📌 Unpin").clicked() {
                                *action = TabUIAction::UnpinTab(tab.id);
                            }
                        } else {
                            if ui.button("📌 Pin").clicked() {
                                *action = TabUIAction::PinTab(tab.id);
                            }
                        }
                    });
                });
            });

    }

    // ── Search overlay ──
    fn render_search_overlay(
        &mut self,
        ctx: &egui::Context,
        mgr: &TabManager,
        action: &mut TabUIAction,
    ) {
        egui::Window::new("Tab Search")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, -100.0))
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.label("Search open tabs (title or URL):");
                ui.text_edit_singleline(&mut self.search_query);

                ui.separator();

                if !self.search_query.is_empty() {
                    let results = mgr.search_tabs(&self.search_query);
                    for result in results.iter().take(10) {
                        let label = if result.is_pinned {
                            format!("📌 {}", result.title)
                        } else {
                            result.title.clone()
                        };
                        if ui.button(label).clicked() {
                            *action = TabUIAction::SwitchTab(result.tab_id);
                            self.search_open = false;
                            self.search_query.clear();
                        }
                        ui.label(
                            RichText::new(&result.url)
                                .small()
                                .color(Color32::from_gray(150)),
                        );
                        ui.separator();
                    }
                    if results.is_empty() {
                        ui.label("No matching tabs.");
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        self.search_open = false;
                        self.search_query.clear();
                    }
                });
            });
    }

    // ── Recently closed overlay ──
    fn render_recently_closed_overlay(
        &mut self,
        ctx: &egui::Context,
        mgr: &mut TabManager,
        action: &mut TabUIAction,
    ) {
        egui::Window::new("Recently Closed Tabs")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, -50.0))
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.label("Restore a recently closed tab:");

                ui.separator();

                let entries = mgr.recently_closed_list();
                if entries.is_empty() {
                    ui.label("No recently closed tabs.");
                } else {
                    for entry in entries.iter() {
                        ui.horizontal(|ui| {
                            if ui.button(&entry.tab.title).clicked() {
                                *action = TabUIAction::RestoreClosedTab;
                                self.recently_closed_open = false;
                            }
                            ui.label(
                                RichText::new(&entry.tab.url)
                                    .small()
                                    .color(Color32::from_gray(150)),
                            );
                        });
                        ui.separator();
                    }
                }

                ui.separator();
                if ui.button("Close").clicked() {
                    self.recently_closed_open = false;
                }
            });
    }

    // ── Group creation dialog ──
    fn render_group_dialog(
        &mut self,
        ctx: &egui::Context,
        mgr: &mut TabManager,
        action: &mut TabUIAction,
    ) {
        egui::Window::new("New Tab Group")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(300.0);
                ui.label("Group name:");
                ui.text_edit_singleline(&mut self.group_dialog_name);

                ui.label("Color:");
                egui::Grid::new("group_colors").show(ui, |ui| {
                    for color in GroupColor::variants() {
                        let hex = color.as_hex();
                        let c = parse_hex_color(hex);
                        let selected = self.group_dialog_color == color;
                        if ui
                            .selectable_label(
                                selected,
                                RichText::new("■").color(c).strong(),
                            )
                            .clicked()
                        {
                            self.group_dialog_color = color;
                        }
                        ui.end_row();
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.group_dialog_open = false;
                        self.group_dialog_name.clear();
                    }
                    if ui.button("Create Group").clicked() {
                        if !self.group_dialog_name.trim().is_empty() {
                            *action = TabUIAction::CreateGroup(
                                self.group_dialog_name.clone(),
                                self.group_dialog_color,
                            );
                            self.group_dialog_open = false;
                            self.group_dialog_name.clear();
                        }
                    }
                });
            });
    }
}

fn parse_hex_color(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color32::from_rgb(66, 133, 244);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(66);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(133);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(244);
    Color32::from_rgb(r, g, b)
}
