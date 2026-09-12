use std::ffi::{c_char, c_void};

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    Fullscreen,
    DropRain,
}

pub struct KeyboardState {
    state: *mut c_void,
}

impl KeyboardState {
    pub fn new(keymap: &[u8]) -> Result<Self, &'static str> {
        let keymap = keymap.strip_suffix(&[0]).unwrap_or(keymap);
        unsafe {
            let context = xkb_context_new(0);
            if context.is_null() {
                return Err("could not create XKB context");
            }
            let map =
                xkb_keymap_new_from_buffer(context, keymap.as_ptr().cast(), keymap.len(), 1, 0);
            xkb_context_unref(context);
            if map.is_null() {
                return Err("could not compile keyboard keymap");
            }
            let state = xkb_state_new(map);
            xkb_keymap_unref(map);
            if state.is_null() {
                return Err("could not create keyboard state");
            }
            Ok(Self { state })
        }
    }

    pub fn update_modifiers(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        unsafe {
            xkb_state_update_mask(self.state, depressed, latched, locked, 0, 0, group);
        }
    }

    pub fn action(&self, wayland_key: u32) -> Option<Action> {
        // Wayland sends evdev codes; XKB keycodes have an offset of eight.
        let keycode = wayland_key.checked_add(8)?;
        let keysym = unsafe { xkb_state_key_get_one_sym(self.state, keycode) };
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
        unsafe { xkb_state_unref(self.state) };
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
