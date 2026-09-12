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

// These tests render to EGL pbuffers without opening a desktop window. Run with
// LIBGL_ALWAYS_SOFTWARE=true cargo test -- --include-ignored --test-threads=1
// on machines providing Mesa's surfaceless EGL platform.
unsafe fn headless_window(width: i32, height: i32) -> WaylandWindow {
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
        8,
        egl::EGL_GREEN_SIZE,
        8,
        egl::EGL_BLUE_SIZE,
        8,
        egl::EGL_ALPHA_SIZE,
        8,
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

unsafe extern "C" {
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
