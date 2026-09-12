//! Optional HDR output: preserve the legacy scene in an FP16 framebuffer,
//! then encode opaque BT.2020/PQ pixels into a 10-bit or FP16 EGL window buffer.

mod protocol;

use super::{c_char, c_int, c_void, egl, gl, wayland, PollFd, POLLIN};
use std::cell::{Cell, RefCell};
use std::ffi::CStr;
use std::os::fd::FromRawFd;
use std::ptr;
use std::time::{Duration, Instant};
use wayland::{WlDisplay, WlInterface, WlProxy, WlRegistry, WlSurface};

const REFERENCE_WHITE: u32 = 203;
const MAX_HIGHLIGHT: f32 = 600.0;
const TF_PQ: u32 = 11;
const PRIMARIES_BT2020: u32 = 6;
const FEATURE_PARAMETRIC: u32 = 1;
const FEATURE_LUMINANCES: u32 = 4;
const FEATURE_MASTERING: u32 = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Auto,
    On,
    Off,
}

impl Mode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            _ => Err(format!("--hdr requires auto, on, or off: got {value:?}")),
        }
    }
}

#[derive(Default)]
struct Events {
    manager: Cell<*mut WlProxy>,
    capabilities_done: Cell<bool>,
    intents: Cell<u32>,
    features: Cell<u32>,
    transfer_functions: Cell<u32>,
    primaries: Cell<u32>,
    description_status: Cell<u8>,
    failure: RefCell<String>,
    info_done: Cell<bool>,
    maximum: Cell<u32>,
    reference: Cell<u32>,
    target: Cell<u32>,
}

pub struct ColorManagement {
    registry: *mut WlRegistry,
    surface: *mut WlProxy,
    feedback: *mut WlProxy,
    description: *mut WlProxy,
    preferred: *mut WlProxy,
    info: *mut WlProxy,
    events: Box<Events>,
    peak: f32,
}

impl ColorManagement {
    pub unsafe fn new(
        display: *mut WlDisplay,
        surface: *mut WlSurface,
        mode: Mode,
    ) -> Result<Self, String> {
        let mut colors = Self {
            registry: super::display_get_registry(display),
            surface: ptr::null_mut(),
            feedback: ptr::null_mut(),
            description: ptr::null_mut(),
            preferred: ptr::null_mut(),
            info: ptr::null_mut(),
            events: Box::default(),
            peak: MAX_HIGHLIGHT,
        };
        let data = (&*colors.events as *const Events).cast_mut().cast();
        listen(
            colors.registry.cast(),
            (&REGISTRY as *const wayland::WlRegistryListener).cast(),
            data,
        )?;
        if wayland::wl_display_roundtrip(display) < 0 {
            return Err("color-management registry roundtrip failed".into());
        }
        let manager = colors.events.manager.get();
        if manager.is_null() {
            return Err("compositor does not advertise wp_color_manager_v1".into());
        }
        wait(display, || colors.events.capabilities_done.get())?;
        let e = &colors.events;
        if !has(e.features.get(), FEATURE_PARAMETRIC)
            || !has(e.features.get(), FEATURE_LUMINANCES)
            || !has(e.transfer_functions.get(), TF_PQ)
            || !has(e.primaries.get(), PRIMARIES_BT2020)
            || !has(e.intents.get(), 0)
        {
            return Err("compositor lacks parametric BT.2020/PQ color management".into());
        }
        let p = protocol::interfaces();
        colors.surface = wayland::wl_proxy_marshal_flags(
            manager,
            2,
            p.surface,
            1,
            0,
            ptr::null_mut::<c_void>(),
            surface,
        );
        if colors.surface.is_null() {
            return Err("could not create a color-management surface".into());
        }
        colors.feedback = wayland::wl_proxy_marshal_flags(
            manager,
            3,
            p.feedback,
            1,
            0,
            ptr::null_mut::<c_void>(),
            surface,
        );
        listen(
            colors.feedback,
            (&FEEDBACK as *const FeedbackListener).cast(),
            data,
        )?;
        colors.preferred = construct(colors.feedback, 2, p.description, 0);
        listen(
            colors.preferred,
            (&DESCRIPTION as *const DescriptionListener).cast(),
            data,
        )?;
        wait(display, || e.description_status.get() != 0)?;
        if e.description_status.get() == 1 {
            colors.info = construct(colors.preferred, 1, p.info, 0);
            listen(colors.info, (&INFO as *const InfoListener).cast(), data)?;
            wait(display, || e.info_done.get())?;
            let maximum = if e.target.get() > 0 {
                e.target.get()
            } else {
                e.maximum.get()
            };
            let reference = e.reference.get();
            let headroom = reference > 0 && maximum > reference;
            if mode == Mode::Auto && !headroom {
                return Err("the surface's preferred color description has no HDR headroom".into());
            }
            if headroom {
                colors.peak = (REFERENCE_WHITE as f32 * maximum as f32 / reference as f32)
                    .clamp(REFERENCE_WHITE as f32, MAX_HIGHLIGHT);
            }
        } else if mode == Mode::Auto {
            return Err(format!(
                "could not determine HDR headroom: {}",
                e.failure.borrow()
            ));
        }
        // A version-1 client may not understand a newer preferred description.
        // Explicit --hdr=on can still submit its own supported PQ description.
        destroy(colors.preferred);
        colors.preferred = ptr::null_mut();
        e.description_status.set(0);
        e.failure.borrow_mut().clear();
        let creator = construct(manager, 5, p.params, 0);
        if creator.is_null() {
            return Err("could not create HDR image parameters".into());
        }
        request_u32(creator, 1, TF_PQ);
        request_u32(creator, 3, PRIMARIES_BT2020);
        wayland::wl_proxy_marshal_flags(
            creator,
            5,
            ptr::null(),
            1,
            0,
            0_u32,
            10_000_u32,
            REFERENCE_WHITE,
        );
        if has(e.features.get(), FEATURE_MASTERING) {
            wayland::wl_proxy_marshal_flags(
                creator,
                7,
                ptr::null(),
                1,
                0,
                0_u32,
                colors.peak.ceil() as u32,
            );
        }
        request_u32(creator, 8, colors.peak.ceil() as u32);
        // create is a destructor for the parameter builder.
        colors.description = construct(creator, 0, p.description, 1);
        listen(
            colors.description,
            (&DESCRIPTION as *const DescriptionListener).cast(),
            data,
        )?;
        wait(display, || e.description_status.get() != 0)?;
        if e.description_status.get() != 1 {
            return Err(format!(
                "compositor rejected HDR image description: {}",
                e.failure.borrow()
            ));
        }
        Ok(colors)
    }

    pub fn peak(&self) -> f32 {
        self.peak
    }

    pub unsafe fn activate(&self) {
        // This state and the PQ buffer are applied together by EGL's next commit.
        wayland::wl_proxy_marshal_flags(
            self.surface,
            1,
            ptr::null(),
            1,
            0,
            self.description,
            0_u32,
        );
    }
}

impl Drop for ColorManagement {
    fn drop(&mut self) {
        unsafe {
            if !self.info.is_null() {
                // The info object's done event is a server-side destructor.
                wayland::wl_proxy_destroy(self.info);
            }
            destroy(self.preferred);
            destroy(self.description);
            destroy(self.surface);
            destroy(self.feedback);
            destroy(self.events.manager.get());
            if !self.registry.is_null() {
                wayland::wl_proxy_destroy(self.registry.cast());
            }
        }
    }
}

fn has(bits: u32, value: u32) -> bool {
    value < 32 && bits & (1_u32 << value) != 0
}

unsafe fn listen(
    proxy: *mut WlProxy,
    listener: *const c_void,
    data: *mut c_void,
) -> Result<(), String> {
    if proxy.is_null() || wayland::wl_proxy_add_listener(proxy, listener, data) != 0 {
        return Err("could not install color-management listener".into());
    }
    Ok(())
}

unsafe fn construct(
    proxy: *mut WlProxy,
    opcode: u32,
    interface: *const WlInterface,
    flags: u32,
) -> *mut WlProxy {
    wayland::wl_proxy_marshal_flags(
        proxy,
        opcode,
        interface,
        1,
        flags,
        ptr::null_mut::<c_void>(),
    )
}

unsafe fn request_u32(proxy: *mut WlProxy, opcode: u32, value: u32) {
    wayland::wl_proxy_marshal_flags(proxy, opcode, ptr::null(), 1, 0, value);
}

unsafe fn destroy(proxy: *mut WlProxy) {
    if !proxy.is_null() {
        wayland::wl_proxy_marshal_flags(proxy, 0, ptr::null(), 1, 1);
    }
}

unsafe fn wait(display: *mut WlDisplay, done: impl Fn() -> bool) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if wayland::wl_display_dispatch_pending(display) < 0
            || wayland::wl_display_get_error(display) != 0
        {
            return Err("Wayland connection failed during HDR setup".into());
        }
        if done() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for color-management information".into());
        }
        if wayland::wl_display_prepare_read(display) != 0 {
            continue;
        }
        wayland::wl_display_flush(display);
        let mut fd = PollFd {
            fd: wayland::wl_display_get_fd(display),
            events: POLLIN,
            revents: 0,
        };
        let ready = super::poll(&mut fd, 1, 100);
        if ready > 0 && fd.revents & POLLIN != 0 {
            if wayland::wl_display_read_events(display) < 0 {
                return Err("could not read Wayland HDR events".into());
            }
        } else {
            wayland::wl_display_cancel_read(display);
            if ready > 0 && fd.revents != 0 {
                return Err("Wayland connection closed during HDR setup".into());
            }
        }
    }
}

unsafe extern "C" fn global(
    data: *mut c_void,
    registry: *mut WlRegistry,
    name: u32,
    interface: *const c_char,
    _version: u32,
) {
    let e = &*data.cast::<Events>();
    if CStr::from_ptr(interface) == c"wp_color_manager_v1" && e.manager.get().is_null() {
        let manager = super::registry_bind(registry, name, protocol::interfaces().manager, 1);
        e.manager.set(manager);
        if !manager.is_null() {
            wayland::wl_proxy_add_listener(
                manager,
                (&MANAGER as *const ManagerListener).cast(),
                data,
            );
        }
    }
}

unsafe extern "C" fn global_remove(_: *mut c_void, _: *mut WlRegistry, _: u32) {}

static REGISTRY: wayland::WlRegistryListener = wayland::WlRegistryListener {
    global: Some(global),
    global_remove: Some(global_remove),
};

type ValueEvent = unsafe extern "C" fn(*mut c_void, *mut WlProxy, u32);
type DoneEvent = unsafe extern "C" fn(*mut c_void, *mut WlProxy);

#[repr(C)]
struct ManagerListener {
    intent: ValueEvent,
    feature: ValueEvent,
    tf: ValueEvent,
    primaries: ValueEvent,
    done: DoneEvent,
}

fn add_bit(bits: &Cell<u32>, value: u32) {
    if value < 32 {
        bits.set(bits.get() | (1 << value));
    }
}

unsafe extern "C" fn intent(data: *mut c_void, _: *mut WlProxy, value: u32) {
    add_bit(&(*data.cast::<Events>()).intents, value);
}
unsafe extern "C" fn feature(data: *mut c_void, _: *mut WlProxy, value: u32) {
    add_bit(&(*data.cast::<Events>()).features, value);
}
unsafe extern "C" fn tf(data: *mut c_void, _: *mut WlProxy, value: u32) {
    add_bit(&(*data.cast::<Events>()).transfer_functions, value);
}
unsafe extern "C" fn primaries(data: *mut c_void, _: *mut WlProxy, value: u32) {
    add_bit(&(*data.cast::<Events>()).primaries, value);
}
unsafe extern "C" fn manager_done(data: *mut c_void, _: *mut WlProxy) {
    (*data.cast::<Events>()).capabilities_done.set(true);
}

static MANAGER: ManagerListener = ManagerListener {
    intent,
    feature,
    tf,
    primaries,
    done: manager_done,
};

#[repr(C)]
struct FeedbackListener {
    changed: ValueEvent,
}
unsafe extern "C" fn ignore_value(_: *mut c_void, _: *mut WlProxy, _: u32) {}
// The compositor maps our fixed content description to each output when moved.
static FEEDBACK: FeedbackListener = FeedbackListener {
    changed: ignore_value,
};

#[repr(C)]
struct DescriptionListener {
    failed: unsafe extern "C" fn(*mut c_void, *mut WlProxy, u32, *const c_char),
    ready: ValueEvent,
}

unsafe extern "C" fn failed(data: *mut c_void, _: *mut WlProxy, _: u32, message: *const c_char) {
    let e = &*data.cast::<Events>();
    *e.failure.borrow_mut() = CStr::from_ptr(message).to_string_lossy().into_owned();
    e.description_status.set(2);
}
unsafe extern "C" fn description_ready(data: *mut c_void, _: *mut WlProxy, _: u32) {
    (*data.cast::<Events>()).description_status.set(1);
}
static DESCRIPTION: DescriptionListener = DescriptionListener {
    failed,
    ready: description_ready,
};

type PrimariesEvent =
    unsafe extern "C" fn(*mut c_void, *mut WlProxy, i32, i32, i32, i32, i32, i32, i32, i32);
#[repr(C)]
struct InfoListener {
    done: DoneEvent,
    icc: unsafe extern "C" fn(*mut c_void, *mut WlProxy, c_int, u32),
    primaries: PrimariesEvent,
    primaries_named: ValueEvent,
    tf_power: ValueEvent,
    tf_named: ValueEvent,
    luminances: unsafe extern "C" fn(*mut c_void, *mut WlProxy, u32, u32, u32),
    target_primaries: PrimariesEvent,
    target_luminance: unsafe extern "C" fn(*mut c_void, *mut WlProxy, u32, u32),
    max_cll: ValueEvent,
    max_fall: ValueEvent,
}
unsafe extern "C" fn info_done(data: *mut c_void, _: *mut WlProxy) {
    (*data.cast::<Events>()).info_done.set(true);
}
unsafe extern "C" fn ignore_icc(_: *mut c_void, _: *mut WlProxy, fd: c_int, _: u32) {
    if fd >= 0 {
        drop(std::fs::File::from_raw_fd(fd));
    }
}
unsafe extern "C" fn ignore_primaries(
    _: *mut c_void,
    _: *mut WlProxy,
    _: i32,
    _: i32,
    _: i32,
    _: i32,
    _: i32,
    _: i32,
    _: i32,
    _: i32,
) {
}
unsafe extern "C" fn luminances(
    data: *mut c_void,
    _: *mut WlProxy,
    _: u32,
    maximum: u32,
    reference: u32,
) {
    let e = &*data.cast::<Events>();
    e.maximum.set(maximum);
    e.reference.set(reference);
}
unsafe extern "C" fn target_luminance(data: *mut c_void, _: *mut WlProxy, _: u32, maximum: u32) {
    (*data.cast::<Events>()).target.set(maximum);
}
static INFO: InfoListener = InfoListener {
    done: info_done,
    icc: ignore_icc,
    primaries: ignore_primaries,
    primaries_named: ignore_value,
    tf_power: ignore_value,
    tf_named: ignore_value,
    luminances,
    target_primaries: ignore_primaries,
    target_luminance,
    max_cll: ignore_value,
    max_fall: ignore_value,
};

pub unsafe fn choose_config(
    display: egl::EGLDisplay,
) -> Result<(egl::EGLConfig, &'static str), String> {
    let extensions = eglQueryString(display, 0x3055); // EGL_EXTENSIONS
    let supports_float = !extensions.is_null()
        && CStr::from_ptr(extensions)
            .to_string_lossy()
            .split_ascii_whitespace()
            .any(|extension| extension == "EGL_EXT_pixel_format_float");
    select_config(supports_float, |attributes| {
        let mut config = ptr::null_mut();
        let mut count = 0;
        if egl::eglChooseConfig(display, attributes.as_ptr(), &mut config, 1, &mut count)
            != egl::EGL_FALSE
            && count > 0
        {
            Some(config)
        } else {
            None
        }
    })
}

pub(super) fn select_config(
    supports_float: bool,
    mut choose: impl FnMut(&[i32]) -> Option<egl::EGLConfig>,
) -> Result<(egl::EGLConfig, &'static str), String> {
    for floating in [false, true] {
        if floating && !supports_float {
            continue;
        }
        let bits = if floating { 16 } else { 10 };
        let mut attributes = vec![
            egl::EGL_SURFACE_TYPE,
            egl::EGL_WINDOW_BIT,
            egl::EGL_RENDERABLE_TYPE,
            egl::EGL_OPENGL_BIT,
            egl::EGL_RED_SIZE,
            bits,
            egl::EGL_GREEN_SIZE,
            bits,
            egl::EGL_BLUE_SIZE,
            bits,
            egl::EGL_ALPHA_SIZE,
            0,
        ];
        if floating {
            // EGL defaults to fixed-point and otherwise excludes FP16 configs.
            attributes.extend([0x3339, 0x333B]); // EGL_COLOR_COMPONENT_TYPE_EXT, FLOAT_EXT
        }
        attributes.push(egl::EGL_NONE);
        if let Some(config) = choose(&attributes) {
            return Ok((config, if floating { "FP16" } else { "10-bit" }));
        }
    }
    Err("driver offers neither 10-bit nor FP16 RGB EGL window configurations".into())
}

const FRAMEBUFFER: u32 = 0x8D40;
const COLOR_ATTACHMENT: u32 = 0x8CE0;
const FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const RGBA16F: i32 = 0x881A;
const FLOAT: u32 = 0x1406;
const CLAMP_TO_EDGE: i32 = 0x812F;
const VERTEX_SHADER: u32 = 0x8B31;
const FRAGMENT_SHADER: u32 = 0x8B30;
const COMPILE_STATUS: u32 = 0x8B81;
const LINK_STATUS: u32 = 0x8B82;

pub struct Renderer {
    program: u32,
    framebuffer: u32,
    texture: u32,
    decoration_texture: u32,
    size: (u32, u32),
}

impl Renderer {
    pub unsafe fn new(peak: f32, size: (u32, u32)) -> Result<Self, String> {
        let extensions = glGetString(0x1F03);
        if extensions.is_null() {
            return Err("could not query OpenGL extensions".into());
        }
        let extensions = CStr::from_ptr(extensions.cast()).to_string_lossy();
        for required in [
            "GL_ARB_framebuffer_object",
            "GL_ARB_texture_float",
            "GL_ARB_texture_non_power_of_two",
        ] {
            if !extensions
                .split_ascii_whitespace()
                .any(|extension| extension == required)
            {
                return Err(format!("OpenGL does not expose {required}"));
            }
        }
        let mut output_bits = i32::MAX;
        for channel in [0x0D52, 0x0D53, 0x0D54] {
            let mut bits = 0;
            gl::glGetIntegerv(channel, &mut bits);
            if bits < 10 {
                return Err("EGL window buffer has fewer than 10 bits per RGB channel".into());
            }
            output_bits = output_bits.min(bits);
        }
        let mut renderer = Self {
            program: 0,
            framebuffer: 0,
            texture: 0,
            decoration_texture: 0,
            size: (0, 0),
        };
        renderer.program = program()?;
        glUseProgram(renderer.program);
        glUniform1i(glGetUniformLocation(renderer.program, c"scene".as_ptr()), 0);
        glUniform1i(
            glGetUniformLocation(renderer.program, c"decorations".as_ptr()),
            1,
        );
        glUniform1f(
            glGetUniformLocation(renderer.program, c"peak".as_ptr()),
            peak,
        );
        let quantum = if output_bits >= 16 {
            0.0
        } else {
            1.0 / ((1_u32 << output_bits) - 1) as f32
        };
        glUniform1f(
            glGetUniformLocation(renderer.program, c"output_quantum".as_ptr()),
            quantum,
        );
        glUseProgram(0);
        gl::glGenTextures(1, &mut renderer.texture);
        gl::glGenTextures(1, &mut renderer.decoration_texture);
        glGenFramebuffers(1, &mut renderer.framebuffer);
        renderer.resize(size)?;
        Ok(renderer)
    }

    unsafe fn resize(&mut self, size: (u32, u32)) -> Result<(), String> {
        if self.size == size {
            return Ok(());
        }
        let mut maximum = 0;
        gl::glGetIntegerv(0x0D33, &mut maximum); // GL_MAX_TEXTURE_SIZE
        if size.0 == 0 || size.1 == 0 || size.0 > maximum as u32 || size.1 > maximum as u32 {
            return Err("HDR framebuffer dimensions exceed the driver's texture limit".into());
        }
        for texture in [self.texture, self.decoration_texture] {
            gl::glBindTexture(gl::GL_TEXTURE_2D, texture);
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_MIN_FILTER,
                gl::GL_LINEAR as i32,
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_MAG_FILTER,
                gl::GL_LINEAR as i32,
            );
            gl::glTexParameteri(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_S, CLAMP_TO_EDGE);
            gl::glTexParameteri(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_T, CLAMP_TO_EDGE);
            gl::glTexImage2D(
                gl::GL_TEXTURE_2D,
                0,
                RGBA16F,
                size.0 as i32,
                size.1 as i32,
                0,
                gl::GL_RGBA,
                FLOAT,
                ptr::null(),
            );
            glBindFramebuffer(FRAMEBUFFER, self.framebuffer);
            glFramebufferTexture2D(FRAMEBUFFER, COLOR_ATTACHMENT, gl::GL_TEXTURE_2D, texture, 0);
            let status = glCheckFramebufferStatus(FRAMEBUFFER);
            glBindFramebuffer(FRAMEBUFFER, 0);
            gl::glBindTexture(gl::GL_TEXTURE_2D, 0);
            let error = glGetError();
            if status != FRAMEBUFFER_COMPLETE || error != 0 {
                return Err(format!(
                    "HDR framebuffer allocation failed (status 0x{status:x}, GL 0x{error:x})"
                ));
            }
        }
        self.size = size;
        Ok(())
    }

    pub unsafe fn begin(&mut self, size: (u32, u32)) -> Result<(), String> {
        self.resize(size)?;
        glBindFramebuffer(FRAMEBUFFER, self.framebuffer);
        glFramebufferTexture2D(
            FRAMEBUFFER,
            COLOR_ATTACHMENT,
            gl::GL_TEXTURE_2D,
            self.texture,
            0,
        );
        Ok(())
    }

    pub unsafe fn begin_decorations(&self) {
        glBindFramebuffer(FRAMEBUFFER, self.framebuffer);
        glFramebufferTexture2D(
            FRAMEBUFFER,
            COLOR_ATTACHMENT,
            gl::GL_TEXTURE_2D,
            self.decoration_texture,
            0,
        );
        gl::glColorMask(1, 1, 1, 1);
        gl::glClearColor(0.0, 0.0, 0.0, 0.0);
        gl::glClear(gl::GL_COLOR_BUFFER_BIT);
        gl::glClearColor(0.0, 0.0, 0.0, 1.0);
    }

    pub unsafe fn present(&self) {
        let mut viewport = [0; 4];
        gl::glGetIntegerv(gl::GL_VIEWPORT, viewport.as_mut_ptr());
        glBindFramebuffer(FRAMEBUFFER, 0);
        gl::glViewport(0, 0, self.size.0 as i32, self.size.1 as i32);
        gl::glDisable(gl::GL_BLEND);
        gl::glDisable(gl::GL_DEPTH_TEST);
        glActiveTexture(0x84C1); // GL_TEXTURE1
        gl::glBindTexture(gl::GL_TEXTURE_2D, self.decoration_texture);
        glActiveTexture(0x84C0); // GL_TEXTURE0
        gl::glBindTexture(gl::GL_TEXTURE_2D, self.texture);
        glUseProgram(self.program);
        gl::glBegin(gl::GL_QUADS);
        gl::glTexCoord2f(0.0, 0.0);
        gl::glVertex3f(-1.0, -1.0, 0.0);
        gl::glTexCoord2f(1.0, 0.0);
        gl::glVertex3f(1.0, -1.0, 0.0);
        gl::glTexCoord2f(1.0, 1.0);
        gl::glVertex3f(1.0, 1.0, 0.0);
        gl::glTexCoord2f(0.0, 1.0);
        gl::glVertex3f(-1.0, 1.0, 0.0);
        gl::glEnd();
        glUseProgram(0);
        glActiveTexture(0x84C1);
        gl::glBindTexture(gl::GL_TEXTURE_2D, 0);
        glActiveTexture(0x84C0);
        gl::glBindTexture(gl::GL_TEXTURE_2D, 0);
        gl::glViewport(viewport[0], viewport[1], viewport[2], viewport[3]);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            if self.framebuffer != 0 {
                glBindFramebuffer(FRAMEBUFFER, 0);
                glDeleteFramebuffers(1, &self.framebuffer);
            }
            if self.texture != 0 {
                gl::glDeleteTextures(1, &self.texture);
            }
            if self.decoration_texture != 0 {
                gl::glDeleteTextures(1, &self.decoration_texture);
            }
            if self.program != 0 {
                glDeleteProgram(self.program);
            }
        }
    }
}

unsafe fn shader(kind: u32, source: &CStr) -> Result<u32, String> {
    let shader = glCreateShader(kind);
    if shader == 0 {
        return Err("could not allocate HDR shader".into());
    }
    glShaderSource(shader, 1, &source.as_ptr(), ptr::null());
    glCompileShader(shader);
    let mut status = 0;
    glGetShaderiv(shader, COMPILE_STATUS, &mut status);
    if status == 0 {
        let mut log = [0_u8; 2048];
        let mut length = 0;
        glGetShaderInfoLog(
            shader,
            log.len() as i32,
            &mut length,
            log.as_mut_ptr().cast(),
        );
        glDeleteShader(shader);
        return Err(format!(
            "HDR shader compilation failed: {}",
            String::from_utf8_lossy(&log[..length.max(0) as usize])
        ));
    }
    Ok(shader)
}

unsafe fn program() -> Result<u32, String> {
    let vertex = shader(VERTEX_SHADER, VERTEX_SOURCE)?;
    let fragment = match shader(FRAGMENT_SHADER, FRAGMENT_SOURCE) {
        Ok(fragment) => fragment,
        Err(error) => {
            glDeleteShader(vertex);
            return Err(error);
        }
    };
    let program = glCreateProgram();
    if program == 0 {
        glDeleteShader(vertex);
        glDeleteShader(fragment);
        return Err("could not allocate HDR shader program".into());
    }
    glAttachShader(program, vertex);
    glAttachShader(program, fragment);
    glLinkProgram(program);
    glDeleteShader(vertex);
    glDeleteShader(fragment);
    let mut status = 0;
    glGetProgramiv(program, LINK_STATUS, &mut status);
    if status == 0 {
        let mut log = [0_u8; 2048];
        let mut length = 0;
        glGetProgramInfoLog(
            program,
            log.len() as i32,
            &mut length,
            log.as_mut_ptr().cast(),
        );
        glDeleteProgram(program);
        return Err(format!(
            "HDR shader link failed: {}",
            String::from_utf8_lossy(&log[..length.max(0) as usize])
        ));
    }
    Ok(program)
}

const VERTEX_SOURCE: &CStr = c"#version 120
varying vec2 uv;
void main() {
    gl_Position = gl_Vertex;
    uv = gl_MultiTexCoord0.xy;
}";

const FRAGMENT_SOURCE: &CStr = c"#version 120
uniform sampler2D scene;
uniform sampler2D decorations;
uniform float peak;
uniform float output_quantum;
varying vec2 uv;
void main() {
    vec3 rgb = clamp(texture2D(scene, uv).rgb, 0.0, 1.0);
    float signal = max(rgb.r, max(rgb.g, rgb.b));
    vec3 linear_rgb = mix(rgb / 12.92, pow((rgb + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), rgb));
    float gain = mix(1.0, peak / 203.0, smoothstep(0.7, 1.0, signal));
    // Decorations are premultiplied sRGB. Composite them in linear light at
    // reference white, without changing the brightness of uncovered rain.
    vec4 overlay = texture2D(decorations, uv);
    vec3 overlay_rgb = clamp(overlay.rgb / max(overlay.a, 0.000001), 0.0, 1.0);
    vec3 overlay_linear = mix(overlay_rgb / 12.92, pow((overlay_rgb + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), overlay_rgb));
    linear_rgb = mix(linear_rgb * gain, overlay_linear, clamp(overlay.a, 0.0, 1.0));
    signal = max(linear_rgb.r, max(linear_rgb.g, linear_rgb.b));
    // Convert linear sRGB/BT.709 to linear BT.2020; preserve the green hue.
    vec3 wide = vec3(dot(linear_rgb, vec3(0.627404, 0.329283, 0.043313)),
                     dot(linear_rgb, vec3(0.069097, 0.919540, 0.011362)),
                     dot(linear_rgb, vec3(0.016391, 0.088013, 0.895595)));
    // SMPTE ST 2084 OETF: absolute luminance normalized to 10000 cd/m2.
    vec3 l = pow(clamp(wide * (203.0 / 10000.0), 0.0, 1.0), vec3(2610.0 / 16384.0));
    vec3 pq = pow((vec3(3424.0 / 4096.0) + (2413.0 / 128.0) * l) /
                  (vec3(1.0) + (2392.0 / 128.0) * l), vec3(2523.0 / 32.0));
    // Dither fixed-point output, not FP16; leave black exactly black.
    float noise = (fract(sin(dot(gl_FragCoord.xy, vec2(12.9898, 78.233))) * 43758.5453) - 0.5) * output_quantum;
    gl_FragColor = vec4(signal > 0.0 ? clamp(pq + noise, 0.0, 1.0) : vec3(0.0), 1.0);
}";

unsafe extern "C" {
    fn eglQueryString(display: egl::EGLDisplay, name: i32) -> *const c_char;
    fn glGetString(name: u32) -> *const u8;
    fn glGetError() -> u32;
    fn glGenFramebuffers(count: i32, framebuffers: *mut u32);
    fn glDeleteFramebuffers(count: i32, framebuffers: *const u32);
    fn glBindFramebuffer(target: u32, framebuffer: u32);
    fn glFramebufferTexture2D(
        target: u32,
        attachment: u32,
        texture_target: u32,
        texture: u32,
        level: i32,
    );
    fn glCheckFramebufferStatus(target: u32) -> u32;
    fn glCreateShader(kind: u32) -> u32;
    fn glShaderSource(shader: u32, count: i32, strings: *const *const c_char, lengths: *const i32);
    fn glCompileShader(shader: u32);
    fn glGetShaderiv(shader: u32, name: u32, value: *mut i32);
    fn glGetShaderInfoLog(shader: u32, size: i32, length: *mut i32, log: *mut c_char);
    fn glDeleteShader(shader: u32);
    fn glCreateProgram() -> u32;
    fn glAttachShader(program: u32, shader: u32);
    fn glLinkProgram(program: u32);
    fn glGetProgramiv(program: u32, name: u32, value: *mut i32);
    fn glGetProgramInfoLog(program: u32, size: i32, length: *mut i32, log: *mut c_char);
    fn glDeleteProgram(program: u32);
    fn glUseProgram(program: u32);
    fn glGetUniformLocation(program: u32, name: *const c_char) -> i32;
    fn glUniform1i(location: i32, value: i32);
    fn glUniform1f(location: i32, value: f32);
    fn glActiveTexture(texture: u32);
}
