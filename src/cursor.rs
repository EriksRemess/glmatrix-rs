use super::{compositor_create_surface, surface_commit, wayland};
use std::cell::Cell;
use std::env;
use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr;

pub struct Cursor {
    theme: *mut c_void,
    surface: *mut wayland::WlSurface,
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
    pub unsafe fn new(
        compositor: *mut wayland::WlCompositor,
        shm: *mut wayland::WlShm,
    ) -> Option<Self> {
        if compositor.is_null() || shm.is_null() {
            return None;
        }
        let name = env::var("XCURSOR_THEME")
            .ok()
            .and_then(|name| CString::new(name).ok());
        let size = env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|size| size.parse::<c_int>().ok())
            .filter(|size| (1..=256).contains(size))
            .unwrap_or(24);
        let theme = wl_cursor_theme_load(
            name.as_ref().map_or(ptr::null(), |name| name.as_ptr()),
            size,
            shm,
        );
        if theme.is_null() {
            eprintln!("could not load pointer cursor theme");
            return None;
        }
        let Some(arrow) = CursorBuffer::load(theme, &[b"default\0", b"left_ptr\0", b"arrow\0"])
        else {
            wl_cursor_theme_destroy(theme);
            eprintln!("pointer cursor theme has no arrow cursor");
            return None;
        };
        let hand = CursorBuffer::load(theme, &[b"pointer\0", b"hand2\0", b"hand1\0"]);
        let surface = compositor_create_surface(compositor);
        if surface.is_null() {
            wl_cursor_theme_destroy(theme);
            return None;
        }
        Some(Self {
            theme,
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
        self.pointing.set(None);
    }

    pub unsafe fn update(&self, pointer: *mut wayland::WlPointer, serial: u32, pointing: bool) {
        if self.pointing.get() != Some(pointing) {
            self.show(pointer, serial, pointing);
        }
    }

    pub unsafe fn show(&self, pointer: *mut wayland::WlPointer, serial: u32, pointing: bool) {
        let image = if pointing {
            self.hand.as_ref().unwrap_or(&self.arrow)
        } else {
            &self.arrow
        };
        // wl_pointer.set_cursor must use the serial of the latest enter event.
        wayland::wl_proxy_marshal_flags(
            pointer.cast(),
            0,
            ptr::null(),
            wayland::wl_proxy_get_version(pointer.cast()),
            0,
            serial,
            self.surface,
            image.hotspot_x,
            image.hotspot_y,
        );
        // Attach and damage the immutable theme buffer in surface coordinates.
        wayland::wl_proxy_marshal_flags(
            self.surface.cast(),
            1,
            ptr::null(),
            wayland::wl_proxy_get_version(self.surface.cast()),
            0,
            image.buffer,
            0_i32,
            0_i32,
        );
        wayland::wl_proxy_marshal_flags(
            self.surface.cast(),
            2,
            ptr::null(),
            wayland::wl_proxy_get_version(self.surface.cast()),
            0,
            0_i32,
            0_i32,
            image.width,
            image.height,
        );
        surface_commit(self.surface);
        self.pointing.set(Some(pointing));
        self.hidden.set(false);
    }
}

impl CursorBuffer {
    unsafe fn load(theme: *mut c_void, names: &[&[u8]]) -> Option<Self> {
        for name in names {
            let cursor = wl_cursor_theme_get_cursor(theme, name.as_ptr().cast());
            if cursor.is_null() || (*cursor).image_count == 0 || (*cursor).images.is_null() {
                continue;
            }
            let image = *(*cursor).images;
            if image.is_null() {
                continue;
            }
            let buffer = wl_cursor_image_get_buffer(image);
            if !buffer.is_null() {
                return Some(Self {
                    buffer,
                    width: (*image).width as c_int,
                    height: (*image).height as c_int,
                    hotspot_x: (*image).hotspot_x as c_int,
                    hotspot_y: (*image).hotspot_y as c_int,
                });
            }
        }
        None
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        unsafe {
            // wl_surface.destroy, including destruction of the client proxy.
            wayland::wl_proxy_marshal_flags(
                self.surface.cast(),
                0,
                ptr::null(),
                wayland::wl_proxy_get_version(self.surface.cast()),
                1,
            );
            wl_cursor_theme_destroy(self.theme);
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
