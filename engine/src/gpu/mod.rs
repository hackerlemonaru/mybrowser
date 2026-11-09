//! Direct GPU rendering module.
//! This is a Vulkan-based renderer using `ash` (raw Vulkan bindings),
//! NOT WebGPU or wgpu. It's the same low-level approach game engines use
//! (e.g., RustVK, ash_renderer) for direct GPU access.
//!
//! Architecture for a browser GPU process:
//!   - Renderer process produces display lists (paint commands)
//!   - GPU process receives them via IPC and issues Vulkan commands
//!   - Each profile gets its own GPU context / command buffer
//!
//! This module provides the skeleton: Vulkan instance, device, swapchain,
//! and a basic render loop. Paint commands from Falco's layout/paint
//! pipeline would be recorded into Vulkan command buffers here.

pub mod vulkan;

/// A frame ready to be rendered — produced by Falco's paint stage.
#[derive(Debug, Clone)]
pub struct PaintFrame {
    /// Display list of paint commands (rects, text, images, etc.)
    pub commands: Vec<PaintCommand>,
    /// Framebuffer dimensions.
    pub width: u32,
    pub height: u32,
}

/// A single paint command from the layout/paint pipeline.
#[derive(Debug, Clone)]
pub enum PaintCommand {
    Rect { x: f32, y: f32, w: f32, h: f32, color: [f32; 4] },
    Text { x: f32, y: f32, text: String, font_size: f32, color: [f32; 4] },
    Image { x: f32, y: f32, w: f32, h: f32, texture_id: u64 },
    Clip { x: f32, y: f32, w: f32, h: f32 },
}

/// GPU context for a single profile. Each profile gets its own
/// Vulkan device/context for isolation.
pub struct GpuContext {
    pub profile_id: String,
    pub renderer: Option<vulkan::VulkanRenderer>,
}

impl GpuContext {
    pub fn new(profile_id: &str) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            renderer: None,
        }
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        self.renderer = Some(vulkan::VulkanRenderer::new()?);
        Ok(())
    }

    pub fn render_frame(&mut self, frame: &PaintFrame) -> anyhow::Result<()> {
        if let Some(r) = &mut self.renderer {
            r.render(frame)?;
        }
        Ok(())
    }
}
