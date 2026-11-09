//! Direct Vulkan renderer skeleton for the browser GPU process.
//!
//! This uses `ash` — raw Vulkan bindings — NOT wgpu, NOT WebGPU.
//! Same low-level approach as game engines (RustVK, ash_renderer, Bevy's Vulkan backend).
//!
//! Architecture:
//!   - Falco's paint stage produces `PaintFrame` (display list)
//!   - This module records those commands into Vulkan command buffers
//!   - Each profile gets its own `VulkanRenderer` / device context for isolation
//!
//! To wire up: pass a `winit::window::Window` raw handle to `VulkanRenderer::new_with_window()`.

use ash::{vk, Instance, Device};
use ash_window;
use raw_window_handle::{RawWindowHandle, HasRawWindowHandle};
use std::ffi::CString;
use winit::window::Window;

/// A Vulkan renderer with direct GPU access.
pub struct VulkanRenderer {
    _entry: ash::Entry,
    _instance: Instance,
    _device: Device,
    _physical_device: vk::PhysicalDevice,
    _graphics_queue: vk::Queue,
    _surface: vk::SurfaceKHR,
    _surface_loader: ash::khr::surface,
    _swapchain_loader: ash::khr::swapchain,
}

impl VulkanRenderer {
    /// Create a Vulkan renderer for a winit window.
    /// This is the game-engine style direct GPU path.
    pub fn new_with_window(window: &Window) -> anyhow::Result<Self> {
        let entry = ash::Entry::linked()?;

        let app_name = CString::new("MyBrowser GPU")?;
        let app_info = vk::ApplicationInfo::builder()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&app_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::make_api_version(1, 3, 0, 0));

        let required_extensions = ash_window::enumerate_required_extensions(window)?;

        let mut create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&required_extensions);

        #[cfg(debug_assertions)]
        {
            let layer_name = CString::new("VK_LAYER_KHRONOS_validation")?;
            create_info = create_info.enabled_layer_names(&[layer_name.as_c_str()]);
        }

        let instance = unsafe { entry.create_instance(&create_info, None) }?;

        let surface = unsafe { ash_window::create_surface(&entry, &instance, window, None)? };
        let surface_loader = ash::khr::surface::new(&entry, &instance);

        // Select physical device.
        let physical_devices = unsafe { instance.enumerate_physical_devices()? };
        let physical_device = physical_devices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No Vulkan-capable GPU found"))?;

        // Find graphics queue family.
        let queue_families = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let graphics_queue_index = queue_families
            .iter()
            .position(|props| props.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .ok_or_else(|| anyhow::anyhow!("No graphics queue family found"))? as u32;

        // Create logical device.
        let queue_create_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(graphics_queue_index)
            .queue_priorities(&[1.0]);

        let device_extensions = [ash::khr::swapchain::name().as_ptr()];

        let device = unsafe {
            instance.create_device(
                physical_device,
                &vk::DeviceCreateInfo::builder()
                    .queue_create_infos(std::slice::from_ref(&queue_create_info.build()))
                    .enabled_extension_names(&device_extensions),
                None,
            )
        }?;

        let graphics_queue = unsafe { device.get_device_queue(graphics_queue_index, 0) };

        let swapchain_loader = ash::khr::swapchain::new(&instance, &device);

        Ok(Self {
            _entry: entry,
            _instance: instance,
            _device: device,
            _physical_device: physical_device,
            _graphics_queue: graphics_queue,
            _surface: surface,
            _surface_loader: surface_loader,
            _swapchain_loader: swapchain_loader,
        })
    }

    /// Submit a paint frame to the GPU.
    /// In a full implementation, this records Falco's paint commands
    /// into Vulkan command buffers and presents to the swapchain.
    pub fn render_frame(&mut self, _frame: &crate::gpu::PaintFrame) -> anyhow::Result<()> {
        // TODO: Record command buffer from paint commands:
        //   - vkCmdBeginRenderPass
        //   - vkCmdBindPipeline, vkCmdDraw for each rect/text/image
        //   - vkCmdEndRenderPass
        //   - vkQueueSubmit, vkQueuePresentKHR
        Ok(())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            self._device.destroy_device(None);
            self._surface_loader.destroy_surface(self._surface, None);
            self._instance.destroy_instance(None);
        }
    }
}

/// GPU context per profile. Each profile gets isolated Vulkan state.
pub struct GpuContext {
    pub profile_id: String,
    pub renderer: Option<VulkanRenderer>,
}

impl GpuContext {
    pub fn new(profile_id: &str) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            renderer: None,
        }
    }

    pub fn init_with_window(&mut self, window: &Window) -> anyhow::Result<()> {
        self.renderer = Some(VulkanRenderer::new_with_window(window)?);
        Ok(())
    }

    pub fn render_frame(&mut self, frame: &crate::gpu::PaintFrame) -> anyhow::Result<()> {
        if let Some(r) = &mut self.renderer {
            r.render_frame(frame)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn gpu_context_creation() {
        let _ctx = super::GpuContext::new("test-profile");
    }
}
