//! OpenGL context creation for Linux (X11/GLX) via raw-window-handle + libloading.
//!
//! Creates a real GLX context so egui_glow can paint to the window,
//! making the MyBrowser GUI visible through noVNC/Xvfb.

use glow::HasContext as _;
use libloading::Library;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::ffi::CString;
use std::os::raw::c_void;
use std::sync::Arc;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

/// Type for glXGetProcAddress function pointer.
type GlxGetProcAddress = unsafe extern "C" fn(*const u8) -> *const c_void;

/// Type for glXSwapBuffers function pointer.
type GlxSwapBuffers = unsafe extern "C" fn(*mut c_void, *mut c_void);

/// OpenGL context wrapper for Linux/X11.
pub struct GlContext {
    _gl_lib: Library,
    gl: Arc<glow::Context>,
    glx_swap_buffers: Option<GlxSwapBuffers>,
    _glx_display: *mut c_void,
    _glx_window: *mut c_void,
}

impl GlContext {
    /// Create a new OpenGL context from a winit Window (X11/GLX on Linux).
    pub unsafe fn new_from_window(window: &Window) -> anyhow::Result<Self> {
        // Load libGL.so.1
        let gl_lib = Library::new("libGL.so.1")
            .map_err(|e| anyhow::anyhow!("Failed to load libGL.so.1: {}", e))?;

        // Get glXGetProcAddress
        let glx_get_proc_address: libloading::Symbol<GlxGetProcAddress> = unsafe {
            gl_lib.get(b"glXGetProcAddress\0")
        }
        .map_err(|e| anyhow::anyhow!("Failed to get glXGetProcAddress: {}", e))?;

        // Get raw display and window handles from winit
        let display_handle = window.display_handle().map_err(|e| {
            anyhow::anyhow!("Failed to get display handle: {}", e)
        })?;
        let window_handle = window.window_handle().map_err(|e| {
            anyhow::anyhow!("Failed to get window handle: {}", e)
        })?;

        let raw_display = display_handle.as_raw();
        let raw_window = window_handle.as_raw();

        let (glx_display, glx_window, glx_screen) = match (raw_display, raw_window) {
            (
                RawDisplayHandle::Xlib(d),
                RawWindowHandle::Xlib(w),
            ) => (
            d.display.map(|p| p.as_ptr() as *mut c_void).unwrap_or(std::ptr::null_mut()),
            w.window as *mut c_void,
            d.screen,
        ),
            (RawDisplayHandle::Wayland(_), _) => {
                anyhow::bail!("Wayland not supported for GLX; use X11")
            }
            _ => anyhow::bail!("Unsupported display/window handle type"),
        };

        // GLX context attributes: request OpenGL 3.3 Core
        let ctx_attrs: [i32; 21] = [
            0x2091, // GLX_CONTEXT_MAJOR_VERSION_ARB
            3,
            0x2092, // GLX_CONTEXT_MINOR_VERSION_ARB
            3,
            0x9126, // GLX_CONTEXT_PROFILE_MASK_ARB
            0x00000001, // GLX_CONTEXT_CORE_PROFILE_BIT_ARB
            5,   // GLX_DOUBLEBUFFER
            1,
            0x8010, // GLX_RENDER_TYPE
            0x00000001, // GLX_RGBA_BIT
            0x8010, // GLX_DRAWABLE_TYPE
            0x00000001, // GLX_WINDOW_BIT
            8,   // GLX_RED_SIZE
            8,
            9,   // GLX_GREEN_SIZE
            8,
            10,  // GLX_BLUE_SIZE
            8,
            11,  // GLX_ALPHA_SIZE
            8,
            0,   // None terminator
        ];

        // Get function pointers via glXGetProcAddress
        let glx_create_context_attribs: *const c_void =
            unsafe { glx_get_proc_address(b"glXCreateContextAttribsARB\0".as_ptr()) };
        let glx_make_current: *const c_void =
            unsafe { glx_get_proc_address(b"glXMakeCurrent\0".as_ptr()) };
        let glx_swap_buffers_ptr: *const c_void =
            unsafe { glx_get_proc_address(b"glXSwapBuffers\0".as_ptr()) };
        let glx_choose_visual: *const c_void =
            unsafe { glx_get_proc_address(b"glXChooseVisual\0".as_ptr()) };
        let glx_create_context_legacy: *const c_void =
            unsafe { glx_get_proc_address(b"glXCreateContext\0".as_ptr()) };
        let glx_destroy_context: *const c_void =
            unsafe { glx_get_proc_address(b"glXDestroyContext\0".as_ptr()) };

        let make_proc = |ptr: *const c_void, name: &str| -> *const c_void {
            if ptr.is_null() {
                eprintln!("[GLX] warning: {} not available", name);
            }
            ptr
        };
        make_proc(glx_make_current, "glXMakeCurrent");

        // Create the GLX context: try modern attribs path first, fall back to
        // a legacy context (needed e.g. under Xvfb / software GLX).
        type GlxCreateContextAttribsARB =
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, bool, *const i32)
                -> *mut c_void;
        let mut glx_context: *mut c_void = std::ptr::null_mut();
        if !glx_create_context_attribs.is_null() {
            let create_ctx: GlxCreateContextAttribsARB =
                unsafe { std::mem::transmute(glx_create_context_attribs) };
            glx_context = unsafe {
                create_ctx(
                    glx_display,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    true,
                    ctx_attrs.as_ptr(),
                )
            };
        } else {
            eprintln!("[GLX] glXCreateContextAttribsARB not available");
        }

        if glx_context.is_null()
            && !glx_choose_visual.is_null()
            && !glx_create_context_legacy.is_null()
        {
            eprintln!("[GLX] falling back to legacy glXCreateContext");
            type GlxChooseVisual =
                unsafe extern "C" fn(*mut c_void, i32, *const i32) -> *mut c_void;
            type GlxCreateContext =
                unsafe extern "C" fn(
                    *mut c_void,
                    *mut c_void,
                    *mut c_void,
                    bool,
                ) -> *mut c_void;
            let choose_visual: GlxChooseVisual =
                unsafe { std::mem::transmute(glx_choose_visual) };
            let create_ctx: GlxCreateContext =
                unsafe { std::mem::transmute(glx_create_context_legacy) };
            let vi_attribs: [i32; 11] = [
                0x800B, // GLX_RGBA
                1,
                5, // GLX_DOUBLEBUFFER
                1,
                8, // GLX_RED_SIZE
                8,
                9, // GLX_GREEN_SIZE
                8,
                10, // GLX_BLUE_SIZE
                8,
                0, // None
            ];
            let vi = unsafe { choose_visual(glx_display, glx_screen, vi_attribs.as_ptr()) };
            if !vi.is_null() {
                glx_context = unsafe { create_ctx(glx_display, vi, std::ptr::null_mut(), true) };
                if !glx_destroy_context.is_null() {
                    type GlxDestroyContext =
                        unsafe extern "C" fn(*mut c_void, *mut c_void);
                    let destroy_ctx: GlxDestroyContext =
                        unsafe { std::mem::transmute(glx_destroy_context) };
                    unsafe { destroy_ctx(glx_display, vi) };
                }
            }
        }

        if glx_context.is_null() {
            anyhow::bail!("Failed to create GLX context");
        }

        // Make the context current
        type GlxMakeCurrent =
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool;
        let make_current: GlxMakeCurrent = unsafe { std::mem::transmute(glx_make_current) };

        let made_current = unsafe {
            make_current(glx_display, glx_window, glx_context)
        };
        if !made_current {
            anyhow::bail!("Failed to make GLX context current");
        }

        // Get swap buffers function
        let swap_buffers: Option<GlxSwapBuffers> =
            if !glx_swap_buffers_ptr.is_null() {
                Some(unsafe { std::mem::transmute(glx_swap_buffers_ptr) })
            } else {
                None
            };

        // Build a GL function loader for glow using glXGetProcAddress
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|name| {
                let c_name = CString::new(name).unwrap();
                let addr =
                    unsafe { glx_get_proc_address(c_name.as_ptr() as *const u8) };
                addr as *const _
            })
        });

        Ok(Self {
            _gl_lib: gl_lib,
            gl,
            glx_swap_buffers: swap_buffers,
            _glx_display: glx_display,
            _glx_window: glx_window,
        })
    }

    /// Swap buffers (present the frame).
    pub unsafe fn swap_buffers(&self) {
        if let Some(swap) = self.glx_swap_buffers {
            unsafe { swap(self._glx_display, self._glx_window) };
        }
    }

    /// Get the glow context.
    pub fn gl(&self) -> &glow::Context {
        &self.gl
    }

    /// Get the glow context as a shared reference (for egui_glow::winit::EguiGlow).
    pub fn gl_arc(&self) -> Arc<glow::Context> {
        self.gl.clone()
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        // Context cleanup handled by library unload
    }
}
