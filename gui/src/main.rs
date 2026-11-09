//! MyBrowser GUI — Chrome-like profile launcher + settings.
//!
//! Shows profile cards (regular, guest, add new) and a settings panel.
//! When a profile is selected, launches the browser engine with that
//! profile's isolated context.

use gui::ProfileSwitcher;
use profile_manager::{GuiChoice, ProfileManager};
use settings::Settings;
use storage::{local_storage, history, bookmarks};
use std::collections::HashMap;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

mod settings;

/// Application mode.
enum AppMode {
    /// Showing profile switcher.
    Switcher,
    /// Showing settings.
    Settings,
    /// Ready to launch with selected profile.
    Launching(String),
}

/// Main application state.
struct App {
    switcher: ProfileSwitcher,
    settings: Settings,
    mode: AppMode,
    mgr: ProfileManager,
}

impl App {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mgr = ProfileManager::new()?;
        let switcher = ProfileSwitcher::new()?;
        let settings = Settings::new();

        Ok(Self {
            switcher,
            settings,
            mode: AppMode::Switcher,
            mgr,
        })
    }

    fn run_ui(&mut self, ctx: &egui::Context) {
        match &self.mode {
            AppMode::Switcher => {
                self.render_switcher(ctx);
            }
            AppMode::Settings => {
                self.render_settings(ctx);
            }
            AppMode::Launching(_) => {
                // Handled externally via take_result.
            }
        }
    }

    fn render_switcher(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("MyBrowser").size(24.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Settings").clicked() {
                        self.mode = AppMode::Settings;
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.switcher.run_ui(ctx);

            // Check for results from the switcher.
            if let Some(choice) = self.switcher.take_result() {
                match choice {
                    GuiChoice::LaunchProfile(id) => {
                        self.mode = AppMode::Launching(id);
                    }
                    GuiChoice::LaunchGuest => {
                        let guest = self.mgr.create_guest().unwrap();
                        self.mode = AppMode::Launching(guest.id);
                    }
                    GuiChoice::CreateProfile => {
                        let name = self.switcher.new_profile_name().to_string();
                        if !name.trim().is_empty() {
                            let profile = self.mgr.create(name.clone()).unwrap();
                            self.mode = AppMode::Launching(profile.id);
                        }
                    }
                    GuiChoice::Quit => {
                        std::process::exit(0);
                    }
                }
            }

            // Auto-launch when in Launching mode.
            if let AppMode::Launching(profile_id) = &self.mode {
                // Signal to the main loop to launch.
                eprintln!("LAUNCH_PROFILE:{}", profile_id);
                std::process::exit(0);
            }
        });
    }

    fn render_settings(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("← Back").clicked() {
                    self.mode = AppMode::Switcher;
                }
                ui.heading(egui::RichText::new("Settings").size(24.0).strong());
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.settings.render(ui, &self.mgr);
        });
    }

    pub fn take_launch_profile(&mut self) -> Option<String> {
        match std::mem::replace(&mut self.mode, AppMode::Switcher) {
            AppMode::Launching(id) => Some(id),
            other => {
                self.mode = other;
                None
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("MyBrowser — Profile Manager")
        .with_inner_size(winit::dpi::LogicalSize::new(900, 700))
        .build(&event_loop)?;

    let mut app = App::new()?;
    let mut egui_ctx = egui::Context::default();
    let mut egui_winit = egui_winit::State::new(
        egui::ViewportId::default(),
        &window,
        None,
        f64::from(window.scale_factor()),
    );

    event_loop.run(move |event, elwt| {
        let _ = elwt;

        let egui_event = egui_winit.on_event(&window, &event);
        if egui_event.consumed {
            return;
        }

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                WindowEvent::Resized(size) => {
                    egui_winit.on_resize(&window, size);
                }
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    egui_winit.on_scale_factor_change(&window, scale_factor);
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let raw_input = egui_winit.take_egui_input(&window);
                egui_ctx.begin_raw(egui_winit.egui_viewport_id(), raw_input);

                app.run_ui(&egui_ctx);

                // Check if we should launch a profile.
                if let Some(profile_id) = app.take_launch_profile() {
                    eprintln!("LAUNCH_PROFILE:{}", profile_id);
                    elwt.exit();
                }

                let paint_jobs = egui_ctx.end_frame();
                let _resources = egui_winit
                    .paint_and_update_textures(&window, &egui_ctx, paint_jobs);
                window.pre_present_notify();
            }
            _ => {}
        }
    })?;

    Ok(())
}
