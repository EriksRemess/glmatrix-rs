use super::{compositor_create_surface, surface_commit, wayland};
use std::cell::Cell;
use std::env;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::{self, NonNull};

pub struct Cursor {
    _theme: CursorTheme,
    surface: NonNull<wayland::WlSurface>,
    arrow: CursorBuffer,
    hand: Option<CursorBuffer>,
    pointing: Cell<Option<bool>>,
    hidden: Cell<bool>,
}

struct CursorBuffer {
    buffer: *mut wayland::WlProxy,
    width: c_int,
    height: c_int,
    hotspot_x: c_int,
    hotspot_y: c_int,
}

impl Cursor {
    /// The compositor and shm proxies must remain live until this cursor is dropped.
    pub unsafe fn new(
        compositor: *mut wayland::WlCompositor,
        shm: *mut wayland::WlShm,
    ) -> Option<Self> {
        if compositor.is_null() || shm.is_null() {
            return None;
        }
        let theme = unsafe { CursorTheme::load(shm) }?;
        let Some(arrow) = theme.buffer(&[c"default", c"left_ptr", c"arrow"]) else {
            eprintln!("pointer cursor theme has no arrow cursor");
            return None;
        };
        let hand = theme.buffer(&[c"pointer", c"hand2", c"hand1"]);
        let surface = NonNull::new(unsafe { compositor_create_surface(compositor) })?;
        Some(Self {
            _theme: theme,
            surface,
            arrow,
            hand,
            pointing: Cell::new(None),
            hidden: Cell::new(false),
        })
    }

    pub unsafe fn hide(&self, pointer: *mut wayland::WlPointer, serial: u32) {
        if self.hidden.replace(true) {
            return;
        }
        unsafe {
            wayland::wl_proxy_marshal_flags(
                pointer.cast(),
                0,
                ptr::null(),
                wayland::wl_proxy_get_version(pointer.cast()),
                0,
                serial,
                ptr::null_mut::<wayland::WlSurface>(),
                0_i32,
                0_i32,
            );
        }
        self.pointing.set(None);
    }

    pub unsafe fn update(&self, pointer: *mut wayland::WlPointer, serial: u32, pointing: bool) {
        if self.pointing.get() != Some(pointing) {
            unsafe { self.show(pointer, serial, pointing) };
        }
    }

    pub unsafe fn show(&self, pointer: *mut wayland::WlPointer, serial: u32, pointing: bool) {
        let image = if pointing {
            self.hand.as_ref().unwrap_or(&self.arrow)
        } else {
            &self.arrow
        };
        unsafe {
            // wl_pointer.set_cursor must use the serial of the latest enter event.
            wayland::wl_proxy_marshal_flags(
                pointer.cast(),
                0,
                ptr::null(),
                wayland::wl_proxy_get_version(pointer.cast()),
                0,
                serial,
                self.surface.as_ptr(),
                image.hotspot_x,
                image.hotspot_y,
            );
            // Attach and damage the immutable theme buffer in surface coordinates.
            wayland::wl_proxy_marshal_flags(
                self.surface.as_ptr().cast(),
                1,
                ptr::null(),
                wayland::wl_proxy_get_version(self.surface.as_ptr().cast()),
                0,
                image.buffer,
                0_i32,
                0_i32,
            );
            wayland::wl_proxy_marshal_flags(
                self.surface.as_ptr().cast(),
                2,
                ptr::null(),
                wayland::wl_proxy_get_version(self.surface.as_ptr().cast()),
                0,
                0_i32,
                0_i32,
                image.width,
                image.height,
            );
            surface_commit(self.surface.as_ptr());
        }
        self.pointing.set(Some(pointing));
        self.hidden.set(false);
    }
}

struct CursorTheme(NonNull<c_void>);

impl CursorTheme {
    unsafe fn load(shm: *mut wayland::WlShm) -> Option<Self> {
        let name = env::var("XCURSOR_THEME")
            .ok()
            .and_then(|name| CString::new(name).ok());
        let size = env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|size| size.parse::<c_int>().ok())
            .filter(|size| (1..=256).contains(size))
            .unwrap_or(24);
        let theme = unsafe {
            wl_cursor_theme_load(
                name.as_ref().map_or(ptr::null(), |name| name.as_ptr()),
                size,
                shm,
            )
        };
        if theme.is_null() {
            eprintln!("could not load pointer cursor theme");
        }
        NonNull::new(theme).map(Self)
    }

    fn buffer(&self, names: &[&CStr]) -> Option<CursorBuffer> {
        for name in names {
            let cursor = unsafe { wl_cursor_theme_get_cursor(self.0.as_ptr(), name.as_ptr()) };
            let Some(cursor) = (unsafe { cursor.as_ref() }) else {
                continue;
            };
            if cursor.image_count == 0 || cursor.images.is_null() {
                continue;
            }
            let image = unsafe { *cursor.images };
            let Some(metadata) = (unsafe { image.as_ref() }) else {
                continue;
            };
            let (width, height, hotspot_x, hotspot_y) = (
                metadata.width as c_int,
                metadata.height as c_int,
                metadata.hotspot_x as c_int,
                metadata.hotspot_y as c_int,
            );
            let buffer = unsafe { wl_cursor_image_get_buffer(image) };
            if !buffer.is_null() {
                return Some(CursorBuffer {
                    buffer,
                    width,
                    height,
                    hotspot_x,
                    hotspot_y,
                });
            }
        }
        None
    }
}

impl Drop for CursorTheme {
    fn drop(&mut self) {
        unsafe { wl_cursor_theme_destroy(self.0.as_ptr()) };
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        unsafe {
            // wl_surface.destroy, including destruction of the client proxy.
            wayland::wl_proxy_marshal_flags(
                self.surface.as_ptr().cast(),
                0,
                ptr::null(),
                wayland::wl_proxy_get_version(self.surface.as_ptr().cast()),
                1,
            );
        }
    }
}

#[repr(C)]
struct CursorImage {
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    delay: u32,
}

#[repr(C)]
struct ThemeCursor {
    image_count: u32,
    images: *mut *mut CursorImage,
    name: *mut c_char,
}

unsafe extern "C" {
    fn wl_cursor_theme_load(
        name: *const c_char,
        size: c_int,
        shm: *mut wayland::WlShm,
    ) -> *mut c_void;
    fn wl_cursor_theme_destroy(theme: *mut c_void);
    fn wl_cursor_theme_get_cursor(theme: *mut c_void, name: *const c_char) -> *mut ThemeCursor;
    fn wl_cursor_image_get_buffer(image: *mut CursorImage) -> *mut wayland::WlProxy;
}
