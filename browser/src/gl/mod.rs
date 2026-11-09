//! OpenGL context creation for the browser window.
//!
//! * **Linux** — real GLX context via `libloading` + `raw-window-handle`, with a
//!   legacy `glXCreateContext` fallback for Xvfb/noVNC.
//! * **Windows** — WGL context via `opengl32`/`gdi32`.
//!
//! Both expose the same shape: `GlContext::new_from_window(&window)`, `gl_arc()`
//! for `egui_glow::winit::EguiGlow`, and `swap_buffers()`.

// ───────────────────────────── Windows (WGL) ─────────────────────────────
#[cfg(windows)]
mod platform {
    use libloading::Library;
    use raw_window_handle::RawWindowHandle;
    use std::ffi::{c_void, CString};
    use std::sync::Arc;
    use winit::raw_window_handle::HasWindowHandle;
    use winit::window::Window;

    type Hdc = *mut c_void;
    type Hglrc = *mut c_void;

    /// `PIXELFORMATDESCRIPTOR` from wingdi.h (40 bytes, matches the C layout).
    #[repr(C)]
    #[allow(non_snake_case)]
    #[derive(Clone, Copy)]
    struct PixelFormatDescriptor {
        nSize: u16,
        nVersion: u16,
        dwFlags: u32,
        iPixelType: u8,
        cColorBits: u8,
        cRedBits: u8,
        cRedShift: u8,
        cGreenBits: u8,
        cGreenShift: u8,
        cBlueBits: u8,
        cBlueShift: u8,
        cAlphaBits: u8,
        cAlphaShift: u8,
        cAccumBits: u8,
        cAccumRedBits: u8,
        cAccumGreenBits: u8,
        cAccumBlueBits: u8,
        cAccumAlphaBits: u8,
        cDepthBits: u8,
        cStencilBits: u8,
        cAuxBuffers: u8,
        iLayerType: u8,
        bReserved: u8,
        dwLayerMask: u32,
        dwVisibleMask: u32,
        dwDamageMask: u32,
    }

    const PFD_DOUBLEBUFFER: u32 = 0x0000_0001;
    const PFD_DRAW_TO_WINDOW: u32 = 0x0000_0004;
    const PFD_SUPPORT_OPENGL: u32 = 0x0000_0020;

    /// OpenGL context wrapper for Windows (WGL).
    pub struct GlContext {
        _opengl32: Library,
        _gdi32: Library,
        _user32: Library,
        gl: Arc<glow::Context>,
        glrc: Hglrc,
        hdc: Hdc,
        swap_buffers_fn: unsafe extern "system" fn(Hdc) -> i32,
    }

    impl GlContext {
        /// Create a WGL OpenGL context for a winit window.
        pub unsafe fn new_from_window(window: &Window) -> anyhow::Result<Self> {
            let handle = window
                .window_handle()
                .map_err(|e| anyhow::anyhow!("Failed to get window handle: {}", e))?;
            let hwnd = match handle.as_raw() {
                RawWindowHandle::Win32(w) => w.hwnd.get() as *mut c_void,
                _ => anyhow::bail!("Unsupported window handle (expected Win32)"),
            };

            let gdi32 = Library::new("gdi32.dll")
                .map_err(|e| anyhow::anyhow!("Failed to load gdi32.dll: {}", e))?;
            let opengl32 = Library::new("opengl32.dll")
                .map_err(|e| anyhow::anyhow!("Failed to load opengl32.dll: {}", e))?;
            // GetDC lives in user32, not gdi32.
            let user32 = Library::new("user32.dll")
                .map_err(|e| anyhow::anyhow!("Failed to load user32.dll: {}", e))?;

            let load = |lib: &Library, name: &str| -> anyhow::Result<*mut c_void> {
                let key = format!("{}\0", name);
                match lib.get::<*mut c_void>(key.as_bytes()) {
                    Ok(sym) => Ok(*sym),
                    Err(e) => anyhow::bail!("GetProcAddress({}) failed: {}", name, e),
                }
            };

            let get_dc: unsafe extern "system" fn(*mut c_void) -> Hdc =
                unsafe { std::mem::transmute(load(&user32, "GetDC")?) };
            let choose_pixel_format: unsafe extern "system" fn(
                Hdc,
                *const PixelFormatDescriptor,
            ) -> i32 = unsafe { std::mem::transmute(load(&gdi32, "ChoosePixelFormat")?) };
            let set_pixel_format: unsafe extern "system" fn(
                Hdc,
                i32,
                *const PixelFormatDescriptor,
            ) -> i32 = unsafe { std::mem::transmute(load(&gdi32, "SetPixelFormat")?) };
            let swap_buffers_fn: unsafe extern "system" fn(Hdc) -> i32 =
                unsafe { std::mem::transmute(load(&gdi32, "SwapBuffers")?) };

            let wgl_create_context: unsafe extern "system" fn(Hdc) -> Hglrc =
                unsafe { std::mem::transmute(load(&opengl32, "wglCreateContext")?) };
            let wgl_make_current: unsafe extern "system" fn(Hdc, Hglrc) -> i32 =
                unsafe { std::mem::transmute(load(&opengl32, "wglMakeCurrent")?) };
            let wgl_get_proc_address: unsafe extern "system" fn(*const i8) -> *const c_void =
                unsafe { std::mem::transmute(load(&opengl32, "wglGetProcAddress")?) };

            let hdc = get_dc(hwnd);
            if hdc.is_null() {
                anyhow::bail!("GetDC returned null");
            }

            let mut pfd: PixelFormatDescriptor = std::mem::zeroed();
            pfd.nSize = std::mem::size_of::<PixelFormatDescriptor>() as u16;
            pfd.nVersion = 1;
            pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
            pfd.iPixelType = 0; // PFD_TYPE_RGBA
            pfd.cColorBits = 32;
            pfd.cAlphaBits = 8;
            pfd.cDepthBits = 24;
            pfd.cStencilBits = 8;
            pfd.iLayerType = 0; // PFD_MAIN_PLANE

            let format = choose_pixel_format(hdc, &pfd);
            if format == 0 {
                anyhow::bail!("ChoosePixelFormat failed");
            }
            if set_pixel_format(hdc, format, &pfd) == 0 {
                eprintln!("[WGL] warning: SetPixelFormat failed (a pixel format may already be set)");
            }

            let glrc = wgl_create_context(hdc);
            if glrc.is_null() {
                anyhow::bail!("wglCreateContext failed");
            }
            if wgl_make_current(hdc, glrc) == 0 {
                anyhow::bail!("wglMakeCurrent failed");
            }

            // Report the driver we actually got: a 1.1 context (Microsoft's GDI
            // generic renderer) cannot back a glow renderer, and knowing the
            // renderer string makes that failure obvious.
            {
                let gl_get_string: unsafe extern "system" fn(u32) -> *const u8 =
                    unsafe { std::mem::transmute(load(&opengl32, "glGetString")?) };
                let read = |token: u32| -> String {
                    unsafe {
                        let ptr = gl_get_string(token);
                        if ptr.is_null() {
                            "unknown".to_string()
                        } else {
                            std::ffi::CStr::from_ptr(ptr as *const i8)
                                .to_string_lossy()
                                .into_owned()
                        }
                    }
                };
                eprintln!(
                    "[WGL] GL_VERSION={} GL_RENDERER={}",
                    read(0x1F02),
                    read(0x1F01)
                );
            }

            let gl = Arc::new(glow::Context::from_loader_function(|name| {
                let Ok(c_name) = CString::new(name) else {
                    return std::ptr::null();
                };
                let proc = wgl_get_proc_address(c_name.as_ptr() as *const i8);
                // wglGetProcAddress signals failure with 0, 1, 2, 3 or -1.
                if !proc.is_null() && proc as usize > 3 {
                    return proc as *const c_void;
                }
                match opengl32.get::<*const c_void>(c_name.as_bytes_with_nul()) {
                    Ok(symbol) => *symbol,
                    Err(_) => std::ptr::null(),
                }
            }));

            eprintln!("[WGL] OpenGL context created");

            Ok(Self {
                _opengl32: opengl32,
                _gdi32: gdi32,
                _user32: user32,
                gl,
                glrc,
                hdc,
                swap_buffers_fn,
            })
        }

        /// Present the frame.
        pub unsafe fn swap_buffers(&self) {
            unsafe { (self.swap_buffers_fn)(self.hdc) };
        }

        /// Borrow the glow context.
        pub fn gl(&self) -> &glow::Context {
            &self.gl
        }

        /// Shared handle for `egui_glow::winit::EguiGlow`.
        pub fn gl_arc(&self) -> Arc<glow::Context> {
            self.gl.clone()
        }
    }

    impl Drop for GlContext {
        fn drop(&mut self) {
            // Context teardown is handled by the process exiting; the WGL
            // function table is not re-entered after the window is dropped.
            let _ = (self.glrc, self.hdc);
        }
    }
}

// ────────────────────────────── Linux (GLX) ──────────────────────────────
#[cfg(target_os = "linux")]
mod platform {
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
            let glx_get_proc_address: libloading::Symbol<GlxGetProcAddress> =
                unsafe { gl_lib.get(b"glXGetProcAddress\0") }
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
                (RawDisplayHandle::Xlib(d), RawWindowHandle::Xlib(w)) => (
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
                    unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, bool) -> *mut c_void;
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
                        type GlxDestroyContext = unsafe extern "C" fn(*mut c_void, *mut c_void);
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
            type GlxMakeCurrent = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool;
            let make_current: GlxMakeCurrent = unsafe { std::mem::transmute(glx_make_current) };

            let made_current = unsafe { make_current(glx_display, glx_window, glx_context) };
            if !made_current {
                anyhow::bail!("Failed to make GLX context current");
            }

            // Get swap buffers function
            let swap_buffers: Option<GlxSwapBuffers> = if !glx_swap_buffers_ptr.is_null() {
                Some(unsafe { std::mem::transmute(glx_swap_buffers_ptr) })
            } else {
                None
            };

            // Build a GL function loader for glow using glXGetProcAddress
            let gl = Arc::new(unsafe {
                glow::Context::from_loader_function(|name| {
                    let c_name = CString::new(name).unwrap();
                    let addr = unsafe { glx_get_proc_address(c_name.as_ptr() as *const u8) };
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
                unsafe { swap(self._glx_display, self._glx_window) }
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
}

pub use platform::GlContext;
