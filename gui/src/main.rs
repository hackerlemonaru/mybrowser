//! Profile switcher GUI entry point.
//! Run this to show the Chrome-like profile picker, then launch the browser.

use gui::ProfileSwitcher;
use profile_manager::GuiChoice;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("MyBrowser — Choose Profile")
        .with_inner_size(winit::dpi::LogicalSize::new(700.0, 550.0))
        .build(&event_loop)?;

    let mut switcher = ProfileSwitcher::new()?;
    let mut egui_ctx = egui::Context::default();
    let mut egui_winit = egui_winit::State::new(
        egui::ViewportId::default(),
        &window,
        None,
        f64::from(window.scale_factor()),
    );

    let mut exit_code: Option<i32> = None;

    event_loop.run(move |event, elwt| {
        let _ = elwt;
        let _ = &event;

        // Handle winit events for egui
        let egui_event = egui_winit.on_event(&window, &event);
        if egui_event.consumed {
            return;
        }

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    exit_code = Some(0);
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

                switcher.run_ui(&egui_ctx);

                // Check for results from GUI interactions
                if let Some(choice) = switcher.take_result() {
                    match choice {
                        GuiChoice::LaunchProfile(id) => {
                            eprintln!("LAUNCH_PROFILE:{}", id);
                            exit_code = Some(0);
                            elwt.exit();
                        }
                        GuiChoice::LaunchGuest => {
                            eprintln!("LAUNCH_GUEST");
                            exit_code = Some(0);
                            elwt.exit();
                        }
                        GuiChoice::CreateProfile => {
                            eprintln!("CREATE_PROFILE:{}", switcher.new_profile_name());
                            exit_code = Some(0);
                            elwt.exit();
                        }
                        GuiChoice::Quit => {
                            exit_code = Some(0);
                            elwt.exit();
                        }
                    }
                }

                let paint_jobs = egui_ctx.end_frame();
                let resources = egui_winit
                    .paint_and_update_textures(&window, &egui_ctx, paint_jobs);
                let _ = resources;
                window.pre_present_notify();
            }
            _ => {}
        }

        if exit_code.is_some() {
            std::process::exit(exit_code.unwrap());
        }
    })?;

    Ok(())
}
