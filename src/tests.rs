use super::*;

fn options() -> Options {
    Options {
        speed: 1.0,
        density: 20.0,
        do_clock: false,
        timefmt: " %l%M%p ".into(),
        do_fog: true,
        do_waves: true,
        do_rotate: true,
        do_texture: false,
        flip_texture: Some(false),
        wireframe: false,
        mode: GlyphMode::Matrix,
        width: 640,
        height: 80,
    }
}

#[test]
fn numeric_arguments_reject_non_finite_values() {
    for argument in ["--speed", "--density"] {
        for value in ["NaN", "inf", "-inf", "1e999", "invalid"] {
            assert!(
                parse_f32_arg(argument, Some(value.into())).is_err(),
                "{argument} {value}"
            );
        }
        assert!(parse_f32_arg(argument, None).is_err());
        assert_eq!(parse_f32_arg(argument, Some("0.5".into())).unwrap(), 0.5);
    }
}

#[test]
fn fullscreen_cursor_hides_only_when_idle_and_returns_on_activity() {
    let mut state = ClientState::new(ptr::null_mut(), 1280, 720);
    state.last_mouse_activity = Instant::now() - Duration::from_secs(2);
    assert!(
        !state.cursor_should_hide(),
        "windowed cursor must stay visible"
    );
    state.fullscreen = true;
    assert!(state.cursor_should_hide());
    state.update_pointer(256, 256);
    assert!(
        !state.cursor_should_hide(),
        "movement must reveal the cursor"
    );
    state.last_mouse_activity = Instant::now() - Duration::from_secs(2);
    let data = (&mut state as *mut ClientState).cast();
    unsafe {
        pointer_button(
            data,
            ptr::null_mut(),
            0,
            0,
            0x111,
            WL_POINTER_BUTTON_STATE_PRESSED,
        );
    }
    assert!(
        !state.cursor_should_hide(),
        "right click must reveal the cursor"
    );
    state.last_mouse_activity = Instant::now() - Duration::from_secs(2);
    unsafe {
        pointer_axis(data, ptr::null_mut(), 0, 0, 256);
    }
    assert!(
        !state.cursor_should_hide(),
        "scrolling must reveal the cursor"
    );
    state.last_mouse_activity = Instant::now() - Duration::from_secs(2);
    state.fullscreen = false;
    assert!(
        !state.cursor_should_hide(),
        "leaving fullscreen must reveal the cursor"
    );
}

fn configure(state: &mut ClientState, width: i32, height: i32, values: &mut [u32]) {
    let mut states = wayland::WlArray {
        size: mem::size_of_val(values),
        alloc: 0,
        data: if values.is_empty() {
            ptr::null_mut()
        } else {
            values.as_mut_ptr().cast()
        },
    };
    unsafe {
        xdg_toplevel_configure(
            (state as *mut ClientState).cast(),
            ptr::null_mut(),
            width,
            height,
            &mut states,
        );
    }
    state.apply_configure_size();
}

#[test]
fn compositor_can_clear_fullscreen_with_empty_states() {
    let mut state = ClientState::new(ptr::null_mut(), 1280, 720);
    configure(&mut state, 1920, 1080, &mut [XDG_TOPLEVEL_STATE_FULLSCREEN]);
    assert!(state.fullscreen);
    assert!(!state.uses_client_decoration());
    configure(&mut state, 1280, 720, &mut []);
    assert!(!state.fullscreen);
    assert!(state.uses_client_decoration());
    assert_eq!((state.width, state.height), (1280, 720));
}

#[test]
fn configure_updates_dimensions_independently() {
    let mut state = ClientState::new(ptr::null_mut(), 1280, 720);
    configure(&mut state, 640, 0, &mut []);
    assert_eq!((state.width, state.height), (640, 720));
    configure(&mut state, 0, 480, &mut []);
    assert_eq!((state.width, state.height), (640, 480));
    configure(&mut state, 0, 0, &mut []);
    assert_eq!((state.width, state.height), (640, 480));
}

const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes { include "evdev+aliases(qwerty)" };
    xkb_types { include "complete" };
    xkb_compatibility { include "complete" };
    xkb_symbols { include "pc+us+fr:2+inet(evdev)" };
};"#;

#[test]
fn shortcuts_follow_layout_and_modifiers() {
    use keyboard::Action;
    let mut keyboard = keyboard::KeyboardState::new(KEYMAP.as_bytes()).unwrap();
    assert_eq!(keyboard.action(16), Some(Action::Quit));
    assert_eq!(keyboard.action(30), None);
    assert_eq!(keyboard.action(33), Some(Action::Fullscreen));
    keyboard.update_modifiers(0, 0, 0, 1);
    assert_eq!(keyboard.action(16), None, "AZERTY A must not quit");
    assert_eq!(
        keyboard.action(30),
        Some(Action::Quit),
        "AZERTY Q must quit"
    );
    keyboard.update_modifiers(1, 0, 0, 1);
    assert_eq!(keyboard.action(30), Some(Action::Quit));
    assert_eq!(keyboard.action(33), Some(Action::Fullscreen));
    keyboard.update_modifiers(0, 0, 2, 1);
    assert_eq!(keyboard.action(30), Some(Action::Quit));
    for key in [14, 111] {
        assert_eq!(keyboard.action(key), Some(Action::DropRain));
    }
    assert_eq!(keyboard.action(1), Some(Action::Quit));
    assert_eq!(keyboard.action(u32::MAX), None);
}

#[test]
fn wayland_keymap_and_modifier_callbacks_install_layout() {
    let path = env::temp_dir().join(format!("glmatrix-keymap-test-{}", process::id()));
    let mut keymap = KEYMAP.as_bytes().to_vec();
    keymap.push(0);
    std::fs::write(&path, &keymap).unwrap();
    let file = File::open(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let mut state = ClientState::new(ptr::null_mut(), 1280, 720);
    let data = (&mut state as *mut ClientState).cast();
    use std::os::fd::IntoRawFd;
    unsafe {
        keyboard_keymap(
            data,
            ptr::null_mut(),
            1,
            file.into_raw_fd(),
            keymap.len() as u32,
        );
        keyboard_modifiers(data, ptr::null_mut(), 0, 0, 0, 0, 1);
        keyboard_key(
            data,
            ptr::null_mut(),
            0,
            0,
            16,
            WL_KEYBOARD_KEY_STATE_PRESSED,
        );
        assert!(state.running);
        keyboard_key(
            data,
            ptr::null_mut(),
            0,
            0,
            14,
            WL_KEYBOARD_KEY_STATE_PRESSED,
        );
        assert!(state.erase_requested.get());
        keyboard_key(
            data,
            ptr::null_mut(),
            0,
            0,
            30,
            WL_KEYBOARD_KEY_STATE_PRESSED,
        );
        assert!(!state.running);
    }
}

#[test]
fn kana_atlas_contains_every_glyph_and_mirrors_exactly() {
    let normal = make_texture_atlas(false);
    let flipped = make_texture_atlas(true);
    for glyph in KANA_GLYPH_START..KANA_GLYPH_START + kana::PATTERNS.len() {
        let bx = glyph % CHAR_COLS * normal.cell;
        let by = (normal.real_rows - glyph / CHAR_COLS - 1) * normal.cell;
        let mut lit = 0;
        for y in 0..normal.cell {
            for x in 0..normal.cell {
                let alpha = normal.data[((by + y) * normal.width + bx + x) * 4 + 3];
                let mirror =
                    flipped.data[((by + y) * flipped.width + bx + normal.cell - x - 1) * 4 + 3];
                assert_eq!(alpha, mirror);
                lit += usize::from(alpha != 0);
            }
        }
        assert!(lit > 0, "empty glyph {glyph}");
    }
}

// These tests render to EGL pbuffers without opening a desktop window. Use the
// README's Testing command to explicitly select Mesa's surfaceless EGL platform
// and software renderer. LIBGL_ALWAYS_SOFTWARE alone does not select Mesa on
// multi-vendor systems; the HDR fixtures require fixed 10-bit pbuffers.
unsafe fn headless_window(width: i32, height: i32) -> WaylandWindow {
    headless_window_with_bits(width, height, 8)
}

unsafe fn headless_window_with_bits(width: i32, height: i32, bits: i32) -> WaylandWindow {
    let display = eglGetPlatformDisplay(0x31DD, ptr::null_mut(), ptr::null());
    assert!(!display.is_null(), "surfaceless EGL display unavailable");
    assert_ne!(
        egl::eglInitialize(display, ptr::null_mut(), ptr::null_mut()),
        0
    );
    assert_ne!(egl::eglBindAPI(egl::EGL_OPENGL_API), 0);
    let attributes = [
        egl::EGL_SURFACE_TYPE,
        1,
        egl::EGL_RENDERABLE_TYPE,
        egl::EGL_OPENGL_BIT,
        egl::EGL_RED_SIZE,
        bits,
        egl::EGL_GREEN_SIZE,
        bits,
        egl::EGL_BLUE_SIZE,
        bits,
        egl::EGL_ALPHA_SIZE,
        if bits >= 10 { 2 } else { 8 },
        egl::EGL_NONE,
    ];
    let mut config = ptr::null_mut();
    let mut count = 0;
    assert_ne!(
        egl::eglChooseConfig(display, attributes.as_ptr(), &mut config, 1, &mut count),
        0
    );
    assert!(count > 0);
    let context = create_egl_context(display, config).unwrap();
    let size = [0x3057, width, 0x3056, height, egl::EGL_NONE];
    let surface = eglCreatePbufferSurface(display, config, size.as_ptr());
    assert!(!surface.is_null());
    assert_ne!(egl::eglMakeCurrent(display, surface, surface, context), 0);
    WaylandWindow {
        state: Box::new(ClientState::new(
            ptr::null_mut(),
            width as u32,
            height as u32,
        )),
        egl_display: display,
        egl_surface: surface,
        egl_context: context,
        hdr: None,
        hdr_colors: None,
    }
}

fn pixels(width: i32, height: i32) -> Vec<u8> {
    let mut bytes = vec![0; (width * height * 4) as usize];
    unsafe {
        glReadPixels(
            0,
            0,
            width,
            height,
            gl::GL_RGBA,
            gl::GL_UNSIGNED_BYTE,
            bytes.as_mut_ptr().cast(),
        );
        assert_eq!(glGetError(), 0);
    }
    bytes
}

#[test]
#[ignore = "requires surfaceless EGL"]
fn decorations_preserve_opacity_and_window_coordinates() {
    let _window = unsafe { headless_window(640, 80) };
    let mut matrix = Matrix::new(options());
    matrix.init_gl();
    for alpha in [1.0, 0.5, 0.0] {
        unsafe {
            gl::glClear(gl::GL_COLOR_BUFFER_BIT);
            gl::glViewport(0, 0, 640, 80);
        }
        matrix.draw_client_border((640, 80), alpha);
        let expected = pixels(640, 80);
        assert!(expected
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
        unsafe {
            gl::glClear(gl::GL_COLOR_BUFFER_BIT);
        }
        matrix.reshape(640, 80);
        let mut before = [0; 4];
        unsafe {
            gl::glGetIntegerv(gl::GL_VIEWPORT, before.as_mut_ptr());
        }
        matrix.draw_client_border((640, 80), alpha);
        assert_eq!(
            pixels(640, 80),
            expected,
            "decorations moved with scene viewport"
        );
        let mut after = [0; 4];
        unsafe {
            gl::glGetIntegerv(gl::GL_VIEWPORT, after.as_mut_ptr());
        }
        assert_eq!(before, after, "scene viewport must be restored");
    }
}

#[test]
#[ignore = "requires surfaceless EGL"]
fn untextured_and_wireframe_rendering_apply_brightness_and_fog() {
    let _window = unsafe { headless_window(64, 64) };
    let mut matrix = Matrix::new(options());
    matrix.init_gl();
    unsafe {
        gl::glViewport(0, 0, 64, 64);
        gl::glMatrixMode(gl::GL_PROJECTION);
        gl::glLoadIdentity();
        gl::glOrtho(0.0, 4.0, 0.0, 4.0, -100.0, 100.0);
        gl::glMatrixMode(gl::GL_MODELVIEW);
        gl::glLoadIdentity();
        gl::glDisable(gl::GL_BLEND);
    }
    for wireframe in [false, true] {
        matrix.options.wireframe = wireframe;
        let draw = |z, brightness| {
            unsafe {
                gl::glClear(gl::GL_COLOR_BUFFER_BIT);
            }
            matrix.draw_glyph(17, false, 1.0, 1.0, z, brightness, &mut 0);
            let pixels = pixels(64, 64);
            assert!(pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255));
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel[1])
                .max()
                .unwrap()
        };
        let bright = draw(0.0, 1.0);
        assert!(bright > 0);
        assert!(draw(0.0, 0.25) < bright / 2, "wave brightness was ignored");
        assert!(draw(-10.0, 1.0) < bright, "depth fog was ignored");
    }
}

#[test]
fn window_dimensions_reject_oversized_values() {
    for name in ["--width", "--height"] {
        for value in ["16385", "500000000", "2147483648", "4294967295", "-1"] {
            assert!(
                parse_u32_arg(name, Some(value.into())).is_err(),
                "{name} {value}"
            );
        }
        for value in [0, 64, 1280, MAX_WINDOW_DIMENSION] {
            assert_eq!(parse_u32_arg(name, Some(value.to_string())).unwrap(), value);
        }
    }
}

#[test]
fn tiny_windows_always_choose_a_valid_resize_edge() {
    let valid = [0, 1, 2, 4, 5, 6, 8, 9, 10];
    let mut state = ClientState::new(ptr::null_mut(), 1280, 720);
    for width in [1, 8, 16, 23, 24, 64, 1280] {
        for height in [1, 8, 16, 23, 24, 64, 720] {
            state.width = width;
            state.height = height;
            for x in [
                0.0,
                width as f64 * 0.25,
                width as f64 * 0.5,
                width as f64 - 1.0,
            ] {
                for y in [
                    0.0,
                    height as f64 * 0.25,
                    height as f64 * 0.5,
                    height as f64 - 1.0,
                ] {
                    state.pointer_x = x;
                    state.pointer_y = y;
                    assert!(
                        valid.contains(&state.resize_edge_at_pointer()),
                        "{width}x{height} at {x},{y}"
                    );
                }
            }
        }
    }
    state.width = 1280;
    state.height = 16;
    state.pointer_x = 500.0;
    state.pointer_y = 2.0;
    assert_eq!(state.resize_edge_at_pointer(), XDG_TOPLEVEL_RESIZE_EDGE_TOP);
    state.pointer_y = 14.0;
    assert_eq!(
        state.resize_edge_at_pointer(),
        XDG_TOPLEVEL_RESIZE_EDGE_BOTTOM
    );
    state.pointer_y = -1.0;
    assert_eq!(
        state.resize_edge_at_pointer(),
        XDG_TOPLEVEL_RESIZE_EDGE_NONE
    );
    state.pointer_y = 0.0;
    state.fullscreen = true;
    assert_eq!(
        state.resize_edge_at_pointer(),
        XDG_TOPLEVEL_RESIZE_EDGE_NONE
    );
}

#[test]
fn fullscreen_restores_saved_size_and_honors_partial_hints() {
    let mut state = ClientState::new(ptr::null_mut(), 1280, 720);
    configure(&mut state, 900, 600, &mut []);
    configure(&mut state, 1920, 1080, &mut [XDG_TOPLEVEL_STATE_FULLSCREEN]);
    configure(&mut state, 2560, 1440, &mut [XDG_TOPLEVEL_STATE_FULLSCREEN]);
    configure(&mut state, 0, 0, &mut []);
    assert_eq!((state.width, state.height), (900, 600));
    configure(&mut state, 1920, 1080, &mut [XDG_TOPLEVEL_STATE_FULLSCREEN]);
    configure(&mut state, 1000, 0, &mut []);
    assert_eq!((state.width, state.height), (1000, 600));
    configure(&mut state, 1920, 1080, &mut [XDG_TOPLEVEL_STATE_FULLSCREEN]);
    configure(&mut state, 0, 700, &mut []);
    assert_eq!((state.width, state.height), (1000, 700));
}

#[test]
fn fullscreen_state_waits_for_surface_configure() {
    let mut state = ClientState::new(ptr::null_mut(), 1280, 720);
    state.requested_fullscreen = Some(true);
    let mut value = XDG_TOPLEVEL_STATE_FULLSCREEN;
    let mut states = wayland::WlArray {
        size: 4,
        alloc: 0,
        data: (&mut value as *mut u32).cast(),
    };
    unsafe {
        xdg_toplevel_configure(
            (&mut state as *mut ClientState).cast(),
            ptr::null_mut(),
            1920,
            1080,
            &mut states,
        );
    }
    assert!(!state.fullscreen);
    assert_eq!((state.width, state.height), (1280, 720));
    state.apply_configure_size();
    assert!(state.fullscreen);
    assert_eq!(state.requested_fullscreen, None);
    assert_eq!((state.windowed_width, state.windowed_height), (1280, 720));
}

// An idle socket peer lets libwayland allocate and release real client proxies
// without opening a desktop window or requiring a running compositor.
struct InputFixture {
    state: Box<ClientState>,
    _peer: std::os::unix::net::UnixStream,
}

impl InputFixture {
    fn finish_fullscreen_request(&mut self) {
        use std::io::Write;
        let callback = self.state.fullscreen_sync;
        assert!(
            !callback.is_null(),
            "fullscreen request must have a sync callback"
        );
        let id = unsafe { wl_proxy_get_id(callback) };
        // wl_callback.done followed by wl_display.delete_id. Dispatch through
        // libwayland so the listener ABI and proxy lifecycle are exercised too.
        let mut wire = Vec::new();
        for word in [id, 12_u32 << 16, 0, 1, (12_u32 << 16) | 1, id] {
            wire.extend_from_slice(&word.to_ne_bytes());
        }
        self._peer.write_all(&wire).unwrap();
        unsafe {
            assert!(wayland::wl_display_dispatch(self.state.display) >= 0);
        }
        assert!(self.state.fullscreen_sync.is_null());
    }

    fn with_toplevel() -> Self {
        let mut fixture = Self::new();
        unsafe {
            let state = &mut fixture.state;
            state.compositor =
                registry_bind(state.registry, 100, &wayland::wl_compositor_interface, 4).cast();
            state.wm_base = registry_bind(state.registry, 101, state.xdg.wm_base, 1);
            state.surface = compositor_create_surface(state.compositor);
            state.xdg_surface =
                xdg_wm_base_get_xdg_surface(state.wm_base, state.xdg.surface, state.surface);
            state.xdg_toplevel = xdg_surface_get_toplevel(state.xdg_surface, state.xdg.toplevel);
            assert!(!state.xdg_toplevel.is_null());
        }
        fixture.fullscreen_requests();
        fixture
    }

    fn fullscreen_requests(&mut self) -> Vec<u16> {
        use std::io::{ErrorKind, Read};
        let object = unsafe {
            assert!(wayland::wl_display_flush(self.state.display) >= 0);
            wl_proxy_get_id(self.state.xdg_toplevel)
        };
        self._peer.set_nonblocking(true).unwrap();
        let mut wire = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            match self._peer.read(&mut buffer) {
                Ok(0) => panic!("Wayland test socket closed"),
                Ok(length) => wire.extend_from_slice(&buffer[..length]),
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => panic!("reading Wayland requests: {error}"),
            }
        }
        let mut requests = Vec::new();
        let mut offset = 0;
        while offset < wire.len() {
            assert!(wire.len() - offset >= 8);
            let id = u32::from_ne_bytes(wire[offset..offset + 4].try_into().unwrap());
            let header = u32::from_ne_bytes(wire[offset + 4..offset + 8].try_into().unwrap());
            let size = (header >> 16) as usize;
            let opcode = header as u16;
            assert!(size >= 8 && offset + size <= wire.len());
            if id == object
                && [
                    XDG_TOPLEVEL_SET_FULLSCREEN as u16,
                    XDG_TOPLEVEL_UNSET_FULLSCREEN as u16,
                ]
                .contains(&opcode)
            {
                requests.push(opcode);
            }
            offset += size;
        }
        requests
    }

    fn new() -> Self {
        use std::os::fd::IntoRawFd;
        let (client, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        unsafe {
            let display = wl_display_connect_to_fd(client.into_raw_fd());
            assert!(!display.is_null());
            let mut state = Box::new(ClientState::new(display, 1280, 720));
            state.registry = display_get_registry(display);
            assert!(!state.registry.is_null());
            Self { state, _peer: peer }
        }
    }

    fn announce(&mut self, name: u32, version: u32) {
        let data = (self.state.as_mut() as *mut ClientState).cast();
        unsafe {
            registry_global(
                data,
                self.state.registry,
                name,
                c"wl_seat".as_ptr(),
                version,
            );
        }
    }

    fn capabilities(&mut self, capabilities: u32) {
        let data = (self.state.as_mut() as *mut ClientState).cast();
        unsafe {
            seat_capabilities(data, self.state.seat, capabilities);
        }
    }
}

impl Drop for InputFixture {
    fn drop(&mut self) {
        unsafe {
            self.state.release_fullscreen_sync();
            self.state.release_pointer();
            self.state.release_keyboard();
            release_input_proxy(self.state.seat.cast(), 3, 5);
            wayland::wl_proxy_destroy(self.state.registry.cast());
            wayland::wl_display_disconnect(self.state.display);
        }
    }
}

#[test]
fn input_capabilities_can_be_removed_and_readded() {
    for version in [2, 5] {
        let mut fixture = InputFixture::new();
        fixture.announce(1, version);
        fixture.capabilities(WL_SEAT_CAPABILITY_POINTER | WL_SEAT_CAPABILITY_KEYBOARD);
        assert!(!fixture.state.pointer.is_null());
        assert!(!fixture.state.keyboard.is_null());
        fixture.state.keyboard_state =
            Some(keyboard::KeyboardState::new(KEYMAP.as_bytes()).unwrap());
        fixture.state.focused = true;
        fixture.state.pointer_down = true;
        fixture.state.press_active = true;
        fixture.state.pointer_enter_serial = Some(123);
        fixture.state.last_click_time = Some(123);
        fixture.capabilities(WL_SEAT_CAPABILITY_KEYBOARD);
        assert!(fixture.state.pointer.is_null());
        assert!(!fixture.state.pointer_down && !fixture.state.press_active);
        assert!(fixture.state.pointer_enter_serial.is_none());
        assert!(fixture.state.last_click_time.is_none());
        assert!(fixture.state.keyboard_state.is_some());
        fixture.capabilities(0);
        assert!(fixture.state.keyboard.is_null());
        assert!(fixture.state.keyboard_state.is_none());
        assert!(!fixture.state.focused);
        fixture.capabilities(WL_SEAT_CAPABILITY_POINTER | WL_SEAT_CAPABILITY_KEYBOARD);
        assert!(!fixture.state.pointer.is_null());
        assert!(!fixture.state.keyboard.is_null());
    }
}

#[test]
fn rapid_fullscreen_toggles_coalesce_while_a_request_is_in_flight() {
    let mut fixture = InputFixture::with_toplevel();
    unsafe {
        fixture.state.toggle_fullscreen();
        fixture.state.send_pending_fullscreen();
    }
    assert_eq!(
        fixture.fullscreen_requests(),
        [XDG_TOPLEVEL_SET_FULLSCREEN as u16]
    );
    unsafe {
        fixture.state.toggle_fullscreen(); // off
        fixture.state.toggle_fullscreen(); // on again
        fixture.state.send_pending_fullscreen();
    }
    assert!(fixture.fullscreen_requests().is_empty());
    configure(
        &mut fixture.state,
        1920,
        1080,
        &mut [XDG_TOPLEVEL_STATE_FULLSCREEN],
    );
    assert_eq!(fixture.state.fullscreen_request_in_flight, Some(true));
    fixture.finish_fullscreen_request();
    unsafe {
        fixture.state.send_pending_fullscreen();
    }
    assert!(fixture.fullscreen_requests().is_empty());
    assert!(fixture.state.fullscreen);
    assert_eq!(fixture.state.fullscreen_request_in_flight, None);
    assert_eq!(fixture.state.requested_fullscreen, None);
    unsafe {
        fixture.state.toggle_fullscreen();
        fixture.state.send_pending_fullscreen();
    }
    assert_eq!(
        fixture.fullscreen_requests(),
        [XDG_TOPLEVEL_UNSET_FULLSCREEN as u16]
    );
}

#[test]
fn queued_fullscreen_exit_waits_for_the_entry_reply() {
    let mut fixture = InputFixture::with_toplevel();
    unsafe {
        fixture.state.toggle_fullscreen();
        fixture.state.send_pending_fullscreen();
        fixture.state.toggle_fullscreen();
        fixture.state.send_pending_fullscreen();
    }
    assert_eq!(
        fixture.fullscreen_requests(),
        [XDG_TOPLEVEL_SET_FULLSCREEN as u16]
    );
    configure(
        &mut fixture.state,
        1920,
        1080,
        &mut [XDG_TOPLEVEL_STATE_FULLSCREEN],
    );
    assert_eq!(fixture.state.requested_fullscreen, Some(false));
    assert!(
        fixture.fullscreen_requests().is_empty(),
        "do not send inside the configure callback"
    );
    fixture.finish_fullscreen_request();
    unsafe {
        fixture.state.send_pending_fullscreen();
    }
    assert_eq!(
        fixture.fullscreen_requests(),
        [XDG_TOPLEVEL_UNSET_FULLSCREEN as u16]
    );
    configure(&mut fixture.state, 0, 0, &mut []);
    fixture.finish_fullscreen_request();
    assert!(!fixture.state.fullscreen);
    assert_eq!(fixture.state.requested_fullscreen, None);
    assert_eq!(fixture.state.fullscreen_request_in_flight, None);
    assert_eq!((fixture.state.width, fixture.state.height), (1280, 720));
}

#[test]
fn rejected_fullscreen_request_does_not_block_retry() {
    let mut fixture = InputFixture::with_toplevel();
    unsafe {
        fixture.state.toggle_fullscreen();
        fixture.state.send_pending_fullscreen();
    }
    assert_eq!(
        fixture.fullscreen_requests(),
        [XDG_TOPLEVEL_SET_FULLSCREEN as u16]
    );
    configure(&mut fixture.state, 1280, 720, &mut []);
    fixture.finish_fullscreen_request();
    assert_eq!(fixture.state.requested_fullscreen, None);
    assert_eq!(fixture.state.fullscreen_request_in_flight, None);
    unsafe {
        fixture.state.toggle_fullscreen();
        fixture.state.send_pending_fullscreen();
    }
    assert_eq!(
        fixture.fullscreen_requests(),
        [XDG_TOPLEVEL_SET_FULLSCREEN as u16]
    );
}

#[test]
fn unrelated_configures_do_not_complete_fullscreen_requests() {
    for initial in [false, true] {
        let mut fixture = InputFixture::with_toplevel();
        fixture.state.fullscreen = initial;
        fixture.state.pending_fullscreen = initial;
        unsafe {
            fixture.state.toggle_fullscreen();
            fixture.state.send_pending_fullscreen();
            fixture.state.toggle_fullscreen(); // return to the initial state
        }
        let first = if initial {
            XDG_TOPLEVEL_UNSET_FULLSCREEN
        } else {
            XDG_TOPLEVEL_SET_FULLSCREEN
        };
        let second = if initial {
            XDG_TOPLEVEL_SET_FULLSCREEN
        } else {
            XDG_TOPLEVEL_UNSET_FULLSCREEN
        };
        assert_eq!(fixture.fullscreen_requests(), [first as u16]);

        // An unrelated activation/resize configure retains the old state.
        let mut states = if initial {
            vec![XDG_TOPLEVEL_STATE_FULLSCREEN]
        } else {
            vec![]
        };
        configure(&mut fixture.state, 1200, 700, &mut states);
        assert_eq!(fixture.state.fullscreen_request_in_flight, Some(!initial));
        assert_eq!(fixture.state.requested_fullscreen, Some(initial));
        unsafe {
            fixture.state.send_pending_fullscreen();
        }
        assert!(fixture.fullscreen_requests().is_empty());

        // The real response still must not release the queue before sync.done.
        let mut states = if initial {
            vec![]
        } else {
            vec![XDG_TOPLEVEL_STATE_FULLSCREEN]
        };
        configure(&mut fixture.state, 1920, 1080, &mut states);
        assert_eq!(fixture.state.fullscreen_request_in_flight, Some(!initial));
        assert_eq!(fixture.state.requested_fullscreen, Some(initial));
        unsafe {
            fixture.state.send_pending_fullscreen();
        }
        assert!(fixture.fullscreen_requests().is_empty());

        fixture.finish_fullscreen_request();
        unsafe {
            fixture.state.send_pending_fullscreen();
        }
        assert_eq!(fixture.fullscreen_requests(), [second as u16]);
        let mut states = if initial {
            vec![XDG_TOPLEVEL_STATE_FULLSCREEN]
        } else {
            vec![]
        };
        configure(&mut fixture.state, 1200, 700, &mut states);
        fixture.finish_fullscreen_request();
        assert_eq!(fixture.state.fullscreen, initial);
        assert_eq!(fixture.state.requested_fullscreen, None);
        assert_eq!(fixture.state.fullscreen_request_in_flight, None);
    }
}

#[test]
fn fullscreen_toggles_in_one_dispatch_batch_cancel_without_requests() {
    let mut fixture = InputFixture::with_toplevel();
    unsafe {
        fixture.state.toggle_fullscreen();
        fixture.state.toggle_fullscreen();
        fixture.state.send_pending_fullscreen();
    }
    assert!(fixture.fullscreen_requests().is_empty());
    assert_eq!(fixture.state.requested_fullscreen, None);
    assert_eq!(fixture.state.fullscreen_request_in_flight, None);
}

#[test]
fn removing_active_seat_selects_an_available_replacement() {
    let mut fixture = InputFixture::new();
    fixture.announce(1, 5);
    fixture.announce(2, 5);
    assert_eq!(fixture.state.seat_global_name, Some(1));
    fixture.capabilities(WL_SEAT_CAPABILITY_POINTER | WL_SEAT_CAPABILITY_KEYBOARD);
    let data = (fixture.state.as_mut() as *mut ClientState).cast();
    unsafe {
        registry_global_remove(data, fixture.state.registry, 1);
    }
    assert_eq!(fixture.state.seat_global_name, Some(2));
    assert!(fixture.state.pointer.is_null() && fixture.state.keyboard.is_null());
    fixture.capabilities(WL_SEAT_CAPABILITY_POINTER | WL_SEAT_CAPABILITY_KEYBOARD);
    assert!(!fixture.state.pointer.is_null() && !fixture.state.keyboard.is_null());
    let data = (fixture.state.as_mut() as *mut ClientState).cast();
    unsafe {
        registry_global_remove(data, fixture.state.registry, 2);
    }
    assert!(fixture.state.seat.is_null());
    assert_eq!(fixture.state.seat_global_name, None);
    fixture.announce(3, 5);
    assert_eq!(fixture.state.seat_global_name, Some(3));
    fixture.capabilities(WL_SEAT_CAPABILITY_POINTER);
    assert!(!fixture.state.pointer.is_null());
}

#[test]
fn hdr_modes_accept_only_documented_values() {
    assert!(matches!(hdr::Mode::parse("auto"), Ok(hdr::Mode::Auto)));
    assert!(matches!(hdr::Mode::parse("on"), Ok(hdr::Mode::On)));
    assert!(matches!(hdr::Mode::parse("off"), Ok(hdr::Mode::Off)));
    for value in ["", "true", "false", "AUTO", "1", "pq"] {
        assert!(hdr::Mode::parse(value).is_err(), "accepted {value:?}");
    }
}

fn config_attribute(attributes: &[i32], name: i32) -> Option<i32> {
    attributes
        .as_chunks::<2>()
        .0
        .iter()
        .find_map(|pair| (pair[0] == name).then_some(pair[1]))
}

#[test]
fn hdr_config_prefers_fixed_point_without_requiring_alpha() {
    let mut calls = 0;
    let config = ptr::dangling_mut::<c_void>();
    let selected = hdr::select_config(true, |attributes| {
        calls += 1;
        assert_eq!(config_attribute(attributes, egl::EGL_RED_SIZE), Some(10));
        assert_eq!(config_attribute(attributes, egl::EGL_GREEN_SIZE), Some(10));
        assert_eq!(config_attribute(attributes, egl::EGL_BLUE_SIZE), Some(10));
        assert_eq!(config_attribute(attributes, egl::EGL_ALPHA_SIZE), Some(0));
        assert_eq!(config_attribute(attributes, 0x3339), None);
        Some(config)
    })
    .unwrap();
    assert_eq!(selected, (config, "10-bit"));
    assert_eq!(calls, 1);
}

#[test]
fn hdr_config_explicitly_requests_fp16_after_fixed_point_fails() {
    let mut calls = 0;
    let config = ptr::dangling_mut::<c_void>();
    let selected = hdr::select_config(true, |attributes| {
        calls += 1;
        if calls == 1 {
            return None;
        }
        assert_eq!(config_attribute(attributes, 0x3339), Some(0x333B));
        for channel in [egl::EGL_RED_SIZE, egl::EGL_GREEN_SIZE, egl::EGL_BLUE_SIZE] {
            assert_eq!(config_attribute(attributes, channel), Some(16));
        }
        assert_eq!(config_attribute(attributes, egl::EGL_ALPHA_SIZE), Some(0));
        Some(config)
    })
    .unwrap();
    assert_eq!(selected, (config, "FP16"));
    assert_eq!(calls, 2);
}

#[test]
fn hdr_config_failure_respects_float_extension_availability() {
    for supports_float in [false, true] {
        let mut calls = 0;
        let result = hdr::select_config(supports_float, |attributes| {
            calls += 1;
            if !supports_float {
                assert_eq!(config_attribute(attributes, 0x3339), None);
            }
            None
        });
        assert!(result.is_err());
        assert_eq!(calls, if supports_float { 2 } else { 1 });
    }
}

fn pixel_from_top(bytes: &[u8], size: (u32, u32), x: u32, y: u32) -> [u8; 4] {
    let offset = ((size.1 - 1 - y) * size.0 + x) as usize * 4;
    bytes[offset..offset + 4].try_into().unwrap()
}

unsafe fn clear_rgba(r: f32, g: f32, b: f32, a: f32) {
    gl::glClearColor(r, g, b, a);
    gl::glClear(gl::GL_COLOR_BUFFER_BIT);
    gl::glClearColor(0.0, 0.0, 0.0, 1.0);
}

#[test]
#[ignore = "requires surfaceless EGL"]
fn hdr_faded_titlebar_does_not_dim_underlying_rain() {
    let size = (128, 80);
    let _window = unsafe { headless_window_with_bits(128, 80, 10) };
    let mut matrix = Matrix::new(options());
    matrix.init_gl();
    let mut renderer = unsafe { hdr::Renderer::new(600.0, size).unwrap() };
    let mut previous = 0;
    for alpha in [1.0, 0.5, 0.0] {
        unsafe {
            renderer.begin(size).unwrap();
            clear_rgba(1.0, 1.0, 1.0, 1.0);
            renderer.begin_decorations();
        }
        matrix.draw_client_border_layer(size, alpha, true);
        unsafe {
            renderer.present();
        }
        let rendered = pixels(128, 80);
        let title = pixel_from_top(&rendered, size, 20, 15);
        let rain = pixel_from_top(&rendered, size, 20, 40);
        assert!(
            title[1] > previous,
            "rain did not brighten as the overlay faded"
        );
        previous = title[1];
        if alpha == 0.0 {
            for channel in 0..3 {
                assert!(
                    title[channel].abs_diff(rain[channel]) <= 1,
                    "faded titlebar left a dim band: {title:?} vs {rain:?}"
                );
            }
        }
        assert_eq!(pixel_from_top(&rendered, size, 4, 40), [0, 0, 0, 255]);
        assert!(rendered
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
    }
}

#[test]
#[ignore = "requires surfaceless EGL"]
fn hdr_pq_pixels_preserve_black_highlights_and_overlay_reference_white() {
    let size = (64, 64);
    let _window = unsafe { headless_window_with_bits(64, 64, 10) };
    let mut matrix = Matrix::new(options());
    matrix.init_gl();
    let mut renderer = unsafe { hdr::Renderer::new(600.0, size).unwrap() };
    let mut values = Vec::new();
    // Reference PQ code values rounded to 8-bit readback: black=0,
    // 600 cd/m2=178, and 203 cd/m2 reference white=148.
    for (scene, overlay_alpha, expected) in [
        (0.0, 0.0, 0_u8),
        (1.0, 0.0, 178),
        (1.0, 1.0, 148),
        (1.0, 0.5, 166),
    ] {
        unsafe {
            renderer.begin(size).unwrap();
            clear_rgba(scene, scene, scene, 1.0);
            renderer.begin_decorations();
            clear_rgba(overlay_alpha, overlay_alpha, overlay_alpha, overlay_alpha);
            renderer.present();
        }
        let pixel = pixel_from_top(&pixels(64, 64), size, 32, 32);
        for channel in &pixel[..3] {
            assert!(
                channel.abs_diff(expected) <= 2,
                "unexpected PQ output: {pixel:?}, expected {expected}"
            );
        }
        assert_eq!(pixel[3], 255);
        values.push(pixel[1]);
    }
    assert!(values[1] > values[3] && values[3] > values[2]);
}

#[test]
#[ignore = "requires surfaceless EGL"]
fn hdr_resize_reallocates_both_layers_and_preserves_viewport() {
    let _window = unsafe { headless_window_with_bits(128, 96, 10) };
    let mut matrix = Matrix::new(options());
    matrix.init_gl();
    let mut renderer = unsafe { hdr::Renderer::new(600.0, (64, 64)).unwrap() };
    for size in [(128, 96), (32, 32), (96, 64)] {
        unsafe {
            gl::glViewport(3, 5, 60, 50);
            renderer.begin(size).unwrap();
            clear_rgba(1.0, 1.0, 1.0, 1.0);
            renderer.begin_decorations();
        }
        matrix.draw_client_border_layer(size, 0.0, true);
        unsafe {
            renderer.present();
        }
        let mut viewport = [0; 4];
        unsafe {
            gl::glGetIntegerv(gl::GL_VIEWPORT, viewport.as_mut_ptr());
        }
        assert_eq!(viewport, [3, 5, 60, 50]);
        let rendered = pixels(size.0 as i32, size.1 as i32);
        assert!(pixel_from_top(&rendered, size, size.0 / 2, size.1 / 2)[1].abs_diff(178) <= 2);
        assert_eq!(
            pixel_from_top(&rendered, size, size.0 / 2, size.1 - 4),
            [0, 0, 0, 255]
        );
    }
    unsafe {
        assert!(renderer.begin((u32::MAX, 64)).is_err());
        renderer.begin((64, 64)).unwrap();
        renderer.begin_decorations();
        renderer.present();
        assert_eq!(glGetError(), 0);
    }
}

unsafe extern "C" {
    fn wl_proxy_get_id(proxy: *mut wayland::WlProxy) -> u32;
    fn wl_display_connect_to_fd(fd: c_int) -> *mut wayland::WlDisplay;
    fn eglGetPlatformDisplay(
        platform: u32,
        native_display: *mut c_void,
        attribs: *const isize,
    ) -> egl::EGLDisplay;
    fn eglCreatePbufferSurface(
        display: egl::EGLDisplay,
        config: egl::EGLConfig,
        attribs: *const i32,
    ) -> egl::EGLSurface;
    fn glReadPixels(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: u32,
        kind: u32,
        data: *mut c_void,
    );
    fn glGetError() -> u32;
}
