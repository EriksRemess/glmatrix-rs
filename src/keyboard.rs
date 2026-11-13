use std::ffi::{c_char, c_void};
use std::ptr::NonNull;

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    Fullscreen,
    DropRain,
}

pub struct KeyboardState {
    state: NonNull<c_void>,
}

impl KeyboardState {
    pub fn new(keymap: &[u8]) -> Result<Self, &'static str> {
        let context = Context::new()?;
        let map = context.compile(keymap.trim_suffix(&[0]))?;
        map.state()
    }

    pub fn update_modifiers(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        unsafe {
            xkb_state_update_mask(self.state.as_ptr(), depressed, latched, locked, 0, 0, group);
        }
    }

    pub fn action(&self, wayland_key: u32) -> Option<Action> {
        // Wayland sends evdev codes; XKB keycodes have an offset of eight.
        let keycode = wayland_key.checked_add(8)?;
        let keysym = unsafe { xkb_state_key_get_one_sym(self.state.as_ptr(), keycode) };
        match keysym {
            0xff1b | 0x71 | 0x51 => Some(Action::Quit),
            0x66 | 0x46 => Some(Action::Fullscreen),
            0xff08 | 0xffff | 0xff9f => Some(Action::DropRain),
            _ => None,
        }
    }
}

impl Drop for KeyboardState {
    fn drop(&mut self) {
        unsafe { xkb_state_unref(self.state.as_ptr()) };
    }
}

struct Context(NonNull<c_void>);

impl Context {
    fn new() -> Result<Self, &'static str> {
        NonNull::new(unsafe { xkb_context_new(0) })
            .map(Self)
            .ok_or("could not create XKB context")
    }

    fn compile(&self, keymap: &[u8]) -> Result<Keymap, &'static str> {
        let map = unsafe {
            xkb_keymap_new_from_buffer(self.0.as_ptr(), keymap.as_ptr().cast(), keymap.len(), 1, 0)
        };
        NonNull::new(map)
            .map(Keymap)
            .ok_or("could not compile keyboard keymap")
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { xkb_context_unref(self.0.as_ptr()) };
    }
}

struct Keymap(NonNull<c_void>);

impl Keymap {
    fn state(&self) -> Result<KeyboardState, &'static str> {
        // libxkbcommon retains the keymap for the new state's lifetime.
        let state = unsafe { xkb_state_new(self.0.as_ptr()) };
        NonNull::new(state)
            .map(|state| KeyboardState { state })
            .ok_or("could not create keyboard state")
    }
}

impl Drop for Keymap {
    fn drop(&mut self) {
        unsafe { xkb_keymap_unref(self.0.as_ptr()) };
    }
}

unsafe extern "C" {
    fn xkb_context_new(flags: u32) -> *mut c_void;
    fn xkb_context_unref(context: *mut c_void);
    fn xkb_keymap_new_from_buffer(
        context: *mut c_void,
        buffer: *const c_char,
        length: usize,
        format: u32,
        flags: u32,
    ) -> *mut c_void;
    fn xkb_keymap_unref(keymap: *mut c_void);
    fn xkb_state_new(keymap: *mut c_void) -> *mut c_void;
    fn xkb_state_unref(state: *mut c_void);
    fn xkb_state_update_mask(
        state: *mut c_void,
        depressed: u32,
        latched: u32,
        locked: u32,
        depressed_layout: u32,
        latched_layout: u32,
        locked_layout: u32,
    ) -> u32;
    fn xkb_state_key_get_one_sym(state: *mut c_void, keycode: u32) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" {
        fn xkb_context_set_log_level(context: *mut c_void, level: u32);
    }

    #[test]
    fn invalid_keymaps_return_compile_errors() {
        let context = Context::new().unwrap();
        const XKB_LOG_LEVEL_CRITICAL: u32 = 10;
        // Suppress expected parser errors only on this test's context.
        unsafe { xkb_context_set_log_level(context.0.as_ptr(), XKB_LOG_LEVEL_CRITICAL) };
        for keymap in [b"".as_slice(), b"invalid keymap", b"invalid keymap\0"] {
            assert!(matches!(
                context.compile(keymap),
                Err("could not compile keyboard keymap")
            ));
        }
    }
}
