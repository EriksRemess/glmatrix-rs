use std::env;
use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::{c_char, c_int, c_long, c_short, c_uint, c_void};
use std::process;
use std::ptr;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CHAR_COLS: usize = 16;
const CHAR_ROWS: usize = 13;
const REAL_CHAR_ROWS: usize = CHAR_ROWS - 2;
const GRID_SIZE: usize = 70;
const GRID_DEPTH: f32 = 35.0;
const WAVE_SIZE: usize = 22;
const SPLASH_RATIO: f32 = 0.7;
const DEFAULT_DELAY: Duration = Duration::from_micros(30_000);
const CLIENT_DECORATION_BORDER: u32 = 8;
const CLIENT_DECORATION_TITLE: u32 = 30;
const WINDOW_TITLE_TEXT: &str = "GLMatrix in Rust";
const RESIZE_GRAB_MARGIN: f64 = 12.0;
const MOVE_DRAG_THRESHOLD: f64 = 5.0;
const DOUBLE_CLICK_MS: u32 = 350;
const DOUBLE_CLICK_DISTANCE: f64 = 10.0;

const MATRIX_ENCODING: [i32; 26] = [
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170,
    171, 172, 173, 174, 175,
];
const DECIMAL_ENCODING: [i32; 10] = [16, 17, 18, 19, 20, 21, 22, 23, 24, 25];
const HEX_ENCODING: [i32; 16] = [
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 33, 34, 35, 36, 37, 38,
];
const BINARY_ENCODING: [i32; 2] = [16, 17];
const DNA_ENCODING: [i32; 4] = [33, 35, 39, 52];

const NICE_VIEWS: [View; 16] = [
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: -20.0 },
    View { x: 0.0, y: 20.0 },
    View { x: 25.0, y: 0.0 },
    View { x: -25.0, y: 0.0 },
    View { x: 25.0, y: 20.0 },
    View { x: -25.0, y: 20.0 },
    View { x: 25.0, y: -20.0 },
    View { x: -25.0, y: -20.0 },
    View { x: 10.0, y: 0.0 },
    View { x: -10.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
    View { x: 0.0, y: 0.0 },
];

const CHAR_MAP: [u8; 256] = [
    96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96,
    96, 96, 96, 96, 96, 96, 96, 96, 0, 1, 2, 96, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
    18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41,
    42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
    66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
    90, 91, 92, 93, 94, 95, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96,
    96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 97, 98, 99, 100, 101, 102, 103,
    104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122,
    123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141,
    142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 96,
    96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96, 96,
    96, 96, 96, 96, 96, 96, 96,
];

#[derive(Clone, Copy)]
struct View {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
enum GlyphMode {
    Matrix,
    Binary,
    Decimal,
    Hexadecimal,
    Dna,
}

impl GlyphMode {
    fn glyphs(self) -> &'static [i32] {
        match self {
            GlyphMode::Matrix => &MATRIX_ENCODING,
            GlyphMode::Binary => &BINARY_ENCODING,
            GlyphMode::Decimal => &DECIMAL_ENCODING,
            GlyphMode::Hexadecimal => &HEX_ENCODING,
            GlyphMode::Dna => &DNA_ENCODING,
        }
    }

    fn flips_texture(self) -> bool {
        matches!(self, GlyphMode::Matrix)
    }

    fn parse(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("matrix") {
            Ok(Self::Matrix)
        } else if value.eq_ignore_ascii_case("bin") || value.eq_ignore_ascii_case("binary") {
            Ok(Self::Binary)
        } else if value.eq_ignore_ascii_case("dec") || value.eq_ignore_ascii_case("decimal") {
            Ok(Self::Decimal)
        } else if value.eq_ignore_ascii_case("hex") || value.eq_ignore_ascii_case("hexadecimal") {
            Ok(Self::Hexadecimal)
        } else if value.eq_ignore_ascii_case("dna") {
            Ok(Self::Dna)
        } else {
            Err(format!(
                "mode must be matrix, binary, decimal, hexadecimal, or dna: got {value:?}"
            ))
        }
    }
}

struct Options {
    speed: f32,
    density: f32,
    do_clock: bool,
    timefmt: String,
    do_fog: bool,
    do_waves: bool,
    do_rotate: bool,
    do_texture: bool,
    wireframe: bool,
    mode: GlyphMode,
    width: u32,
    height: u32,
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut options = Self {
            speed: 1.0,
            density: 20.0,
            do_clock: false,
            timefmt: " %l%M%p ".to_string(),
            do_fog: true,
            do_waves: true,
            do_rotate: true,
            do_texture: true,
            wireframe: false,
            mode: GlyphMode::Matrix,
            width: 1280,
            height: 720,
        };

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Ok(None),
                "-speed" | "--speed" => {
                    options.speed = parse_f32_arg(&arg, args.next())?;
                }
                "-density" | "--density" => {
                    options.density = parse_f32_arg(&arg, args.next())?;
                }
                "-mode" | "--mode" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{arg} requires a value"))?;
                    options.mode = GlyphMode::parse(&value)?;
                }
                "-binary" | "--binary" => options.mode = GlyphMode::Binary,
                "-decimal" | "--decimal" => options.mode = GlyphMode::Decimal,
                "-hexadecimal" | "--hexadecimal" | "-hex" | "--hex" => {
                    options.mode = GlyphMode::Hexadecimal;
                }
                "-dna" | "--dna" => options.mode = GlyphMode::Dna,
                "-clock" | "--clock" => options.do_clock = true,
                "+clock" => options.do_clock = false,
                "-timefmt" | "--timefmt" => {
                    options.timefmt = args
                        .next()
                        .ok_or_else(|| format!("{arg} requires a value"))?;
                }
                "-fog" | "--fog" => options.do_fog = true,
                "+fog" => options.do_fog = false,
                "-waves" | "--waves" => options.do_waves = true,
                "+waves" => options.do_waves = false,
                "-rotate" | "--rotate" => options.do_rotate = true,
                "+rotate" => options.do_rotate = false,
                "-texture" | "--texture" => options.do_texture = true,
                "+texture" => options.do_texture = false,
                "-wireframe" | "--wireframe" => options.wireframe = true,
                "+wireframe" => options.wireframe = false,
                "-width" | "--width" => {
                    options.width = parse_u32_arg(&arg, args.next())?;
                }
                "-height" | "--height" => {
                    options.height = parse_u32_arg(&arg, args.next())?;
                }
                _ => return Err(format!("unknown option {arg:?}; use --help for usage")),
            }
        }

        options.speed = options.speed.clamp(0.05, 20.0);
        options.density = options.density.clamp(0.1, 1000.0);
        options.width = options.width.max(64);
        options.height = options.height.max(64);
        if options.wireframe {
            options.do_texture = false;
        }

        Ok(Some(options))
    }
}

fn parse_f32_arg(name: &str, value: Option<String>) -> Result<f32, String> {
    value
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse()
        .map_err(|_| format!("{name} requires a numeric value"))
}

fn parse_u32_arg(name: &str, value: Option<String>) -> Result<u32, String> {
    value
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse()
        .map_err(|_| format!("{name} requires an integer value"))
}

fn print_help() {
    println!(
        "\
glmatrix-rs

Options:
  -speed N             animation speed, default 1.0
  -density N           coverage density, default 20
  -mode NAME           matrix, binary, decimal, hexadecimal, dna
  -binary              shortcut for -mode binary
  -decimal             shortcut for -mode decimal
  -hexadecimal         shortcut for -mode hexadecimal
  -dna                 shortcut for -mode dna
  -clock / +clock      show/hide local time in strips
  -timefmt FMT         strftime format, default \" %l%M%p \"
  -fog / +fog          enable/disable depth brightness fog
  -waves / +waves      enable/disable brightness waves
  -rotate / +rotate    enable/disable camera auto-rotation
  -texture / +texture  enable/disable textured glyphs
  -wireframe           draw glyph outlines
  -width N             initial window width, default 1280
  -height N            initial window height, default 720

Controls:
  Esc or q             quit
  F                    toggle fullscreen
  left mouse button    pause strip motion while held
  click + drag         move the window
  drag window edge     resize the window
  double click         toggle fullscreen"
    );
}

struct Rng {
    state: u64,
}

impl Rng {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let pid = u64::from(process::id());
        Self {
            state: nanos ^ pid.rotate_left(32) ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn usize(&mut self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            (self.next_u64() % max as u64) as usize
        }
    }

    fn one_in(&mut self, n: usize) -> bool {
        self.usize(n) == 0
    }

    fn frand(&mut self, max: f32) -> f32 {
        let unit = ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32);
        unit * max
    }

    fn bellrand(&mut self, max: f32) -> f32 {
        (self.frand(max) + self.frand(max) + self.frand(max)) / 3.0
    }
}

#[derive(Clone)]
struct Strip {
    x: f32,
    y: f32,
    z: f32,
    dx: f32,
    dy: f32,
    dz: f32,
    erasing: bool,
    spinner_glyph: i32,
    spinner_y: f32,
    spinner_speed: f32,
    glyphs: [i32; GRID_SIZE],
    highlight: [bool; GRID_SIZE],
    spin_speed: i32,
    spin_tick: i32,
    wave_position: i32,
    wave_speed: i32,
    wave_tick: i32,
}

impl Default for Strip {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            erasing: false,
            spinner_glyph: 0,
            spinner_y: 0.0,
            spinner_speed: 0.0,
            glyphs: [0; GRID_SIZE],
            highlight: [false; GRID_SIZE],
            spin_speed: 1,
            spin_tick: 0,
            wave_position: 0,
            wave_speed: 1,
            wave_tick: 0,
        }
    }
}

struct Matrix {
    options: Options,
    rng: Rng,
    texture: gl::GLuint,
    nstrips: usize,
    strips: Vec<Strip>,
    glyph_map: &'static [i32],
    tex_char_width: f32,
    tex_char_height: f32,
    real_char_rows: i32,
    brightness_ramp: [f32; WAVE_SIZE],
    last_view: usize,
    target_view: usize,
    view_x: f32,
    view_y: f32,
    view_steps: i32,
    view_tick: i32,
    auto_tracking: bool,
    track_tick: i32,
    button_down: bool,
}

impl Matrix {
    fn new(options: Options) -> Self {
        let glyph_map = options.mode.glyphs();
        let nstrips = ((options.density * 2.2) as usize).clamp(1, 2000);
        let mut matrix = Self {
            options,
            rng: Rng::new(),
            texture: 0,
            nstrips,
            strips: Vec::with_capacity(nstrips),
            glyph_map,
            tex_char_width: 0.0,
            tex_char_height: 0.0,
            real_char_rows: REAL_CHAR_ROWS as i32,
            brightness_ramp: [0.0; WAVE_SIZE],
            last_view: 0,
            target_view: 0,
            view_x: NICE_VIEWS[0].x,
            view_y: NICE_VIEWS[0].y,
            view_steps: 100,
            view_tick: 0,
            auto_tracking: false,
            track_tick: 0,
            button_down: false,
        };

        for i in 0..WAVE_SIZE {
            let mut j = (WAVE_SIZE - i) as f32 / (WAVE_SIZE - 1) as f32;
            j *= std::f32::consts::FRAC_PI_2;
            j = j.sin();
            matrix.brightness_ramp[i] = 0.2 + j * 0.8;
        }

        for _ in 0..nstrips {
            let strip = matrix.random_strip(true);
            matrix.strips.push(strip);
        }

        matrix
    }

    fn init_gl(&mut self) {
        unsafe {
            gl::glShadeModel(gl::GL_SMOOTH);
            gl::glDisable(gl::GL_DEPTH_TEST);
            gl::glDisable(gl::GL_CULL_FACE);
            gl::glEnable(gl::GL_NORMALIZE);
            gl::glClearColor(0.0, 0.0, 0.0, 1.0);
        }

        if self.options.do_texture {
            self.load_texture();
        }
    }

    fn load_texture(&mut self) {
        let atlas = make_texture_atlas(self.options.mode.flips_texture());
        self.real_char_rows = atlas.real_rows as i32;
        self.tex_char_width = atlas.cell as f32 / atlas.width as f32;
        self.tex_char_height = atlas.cell as f32 / atlas.height as f32;

        unsafe {
            gl::glGenTextures(1, &mut self.texture);
            gl::glPixelStorei(gl::GL_UNPACK_ALIGNMENT, 1);
            gl::glBindTexture(gl::GL_TEXTURE_2D, self.texture);
            gl::glTexImage2D(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA as gl::GLint,
                atlas.width as gl::GLsizei,
                atlas.height as gl::GLsizei,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                atlas.data.as_ptr().cast(),
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_MAG_FILTER,
                gl::GL_LINEAR as gl::GLint,
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_MIN_FILTER,
                gl::GL_LINEAR as gl::GLint,
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_WRAP_S,
                gl::GL_REPEAT as gl::GLint,
            );
            gl::glTexParameteri(
                gl::GL_TEXTURE_2D,
                gl::GL_TEXTURE_WRAP_T,
                gl::GL_REPEAT as gl::GLint,
            );
            gl::glTexEnvi(
                gl::GL_TEXTURE_ENV,
                gl::GL_TEXTURE_ENV_MODE,
                gl::GL_MODULATE as gl::GLint,
            );
        }
    }

    fn reshape(&self, width: u32, height: u32) {
        let mut viewport_height = height.max(1) as i32;
        let viewport_width = width.max(1) as i32;
        let mut y = 0;

        if viewport_width > viewport_height * 5 {
            viewport_height = viewport_width * 9 / 16;
            y = -viewport_height / 2;
        }

        let aspect = viewport_width as f64 / viewport_height.max(1) as f64;
        let near = 1.0_f64;
        let far = 100.0_f64;
        let top = near * (80.0_f64.to_radians() / 2.0).tan();
        let right = top * aspect;

        unsafe {
            gl::glViewport(0, y, viewport_width, viewport_height);
            gl::glMatrixMode(gl::GL_PROJECTION);
            gl::glLoadIdentity();
            gl::glFrustum(-right, right, -top, top, near, far);
            gl::glMatrixMode(gl::GL_MODELVIEW);
            gl::glLoadIdentity();
            gl::glTranslatef(0.0, 0.0, -25.0);
            gl::glClear(gl::GL_COLOR_BUFFER_BIT);
        }
    }

    fn set_button_down(&mut self, down: bool) {
        self.button_down = down;
    }

    fn random_strip(&mut self, initial_erasing: bool) -> Strip {
        let mut strip = Strip {
            x: self.rng.frand(GRID_SIZE as f32) - GRID_SIZE as f32 / 2.0,
            y: GRID_SIZE as f32 / 2.0 + self.rng.bellrand(0.5),
            z: GRID_DEPTH * 0.2 - self.rng.frand(GRID_DEPTH * 0.7),
            dx: 0.0,
            dy: 0.0,
            dz: self.rng.bellrand(0.02) * self.options.speed,
            spinner_y: 0.0,
            spinner_speed: self.rng.bellrand(0.3) * self.options.speed,
            spin_speed: self.rng.bellrand(2.0 / self.options.speed) as i32 + 1,
            wave_speed: self.rng.bellrand(3.0 / self.options.speed) as i32 + 1,
            ..Strip::default()
        };

        let mut time_displayed = false;
        let mut i = 0;
        while i < GRID_SIZE {
            if self.options.do_clock
                && !time_displayed
                && i < GRID_SIZE - 5
                && self.rng.one_in((GRID_SIZE - 5) * 5)
            {
                let text = current_time_text(&self.options.timefmt);
                for b in text.bytes() {
                    if i >= GRID_SIZE {
                        break;
                    }
                    strip.glyphs[i] = i32::from(CHAR_MAP[b as usize]) + 1;
                    strip.highlight[i] = true;
                    i += 1;
                }
                time_displayed = true;
            } else {
                let draw = self.rng.usize(7) != 0;
                let spin = draw && self.rng.one_in(20);
                let mut glyph = if draw {
                    random_visible_glyph(&mut self.rng, self.glyph_map)
                } else {
                    0
                };
                if spin {
                    glyph = -glyph;
                }
                strip.glyphs[i] = glyph;
                strip.highlight[i] = false;
                i += 1;
            }
        }

        strip.spinner_glyph = -random_visible_glyph(&mut self.rng, self.glyph_map);

        if initial_erasing {
            strip.erasing = true;
            strip.spinner_y = self.rng.frand(GRID_SIZE as f32);
            strip.glyphs = [0; GRID_SIZE];
            strip.highlight = [false; GRID_SIZE];
        }

        strip
    }

    fn tick_strip(&mut self, index: usize) {
        if self.button_down {
            return;
        }

        let mut reset = false;
        {
            let strip = &mut self.strips[index];
            strip.x += strip.dx;
            strip.y += strip.dy;
            strip.z += strip.dz;
            if strip.z > GRID_DEPTH * SPLASH_RATIO {
                reset = true;
            }
        }
        if reset {
            self.strips[index] = self.random_strip(false);
            return;
        }

        {
            let strip = &mut self.strips[index];
            strip.spinner_y += strip.spinner_speed;
            if strip.spinner_y >= GRID_SIZE as f32 {
                if strip.erasing {
                    reset = true;
                } else {
                    strip.erasing = true;
                    strip.spinner_y = 0.0;
                    strip.spinner_speed /= 2.0;
                }
            }
        }
        if reset {
            self.strips[index] = self.random_strip(false);
            return;
        }

        let glyph_map = self.glyph_map;
        let rng = &mut self.rng;
        let strip = &mut self.strips[index];

        strip.spin_tick += 1;
        if strip.spin_tick > strip.spin_speed {
            strip.spin_tick = 0;
            strip.spinner_glyph = -random_visible_glyph(rng, glyph_map);
            for glyph in &mut strip.glyphs {
                if *glyph < 0 {
                    *glyph = -random_visible_glyph(rng, glyph_map);
                    if rng.one_in(800) {
                        *glyph = -*glyph;
                    }
                }
            }
        }

        strip.wave_tick += 1;
        if strip.wave_tick > strip.wave_speed {
            strip.wave_tick = 0;
            strip.wave_position += 1;
            if strip.wave_position >= WAVE_SIZE as i32 {
                strip.wave_position = 0;
            }
        }
    }

    fn draw_frame(&mut self, window: &WaylandWindow) {
        unsafe {
            gl::glClear(gl::GL_COLOR_BUFFER_BIT | gl::GL_DEPTH_BUFFER_BIT);
            gl::glPushMatrix();
        }

        if self.options.do_texture {
            unsafe {
                gl::glEnable(gl::GL_TEXTURE_2D);
                gl::glEnable(gl::GL_BLEND);
                gl::glBlendFunc(gl::GL_SRC_ALPHA, gl::GL_ONE);
                gl::glBindTexture(gl::GL_TEXTURE_2D, self.texture);
            }
        } else {
            unsafe {
                gl::glDisable(gl::GL_TEXTURE_2D);
                gl::glDisable(gl::GL_BLEND);
            }
        }

        if self.options.do_rotate {
            unsafe {
                gl::glRotatef(self.view_x, 1.0, 0.0, 0.0);
                gl::glRotatef(self.view_y, 0.0, 1.0, 0.0);
            }
        }

        let mut order: Vec<usize> = (0..self.nstrips).collect();
        order.sort_by(|a, b| {
            self.strips[*a]
                .z
                .partial_cmp(&self.strips[*b].z)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut polygon_count = 0_u64;
        for index in order {
            self.tick_strip(index);
            self.draw_strip(&self.strips[index], &mut polygon_count);
        }

        self.auto_track();

        unsafe {
            gl::glPopMatrix();
        }
        if window.uses_client_decoration() {
            self.draw_client_border(window.size());
        }

        unsafe {
            gl::glFinish();
        }
        window.swap_buffers();
    }

    fn draw_client_border(&self, (width, height): (u32, u32)) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let border = CLIENT_DECORATION_BORDER as f32;
        let title = CLIENT_DECORATION_TITLE as f32;

        unsafe {
            gl::glDisable(gl::GL_TEXTURE_2D);
            gl::glEnable(gl::GL_BLEND);
            gl::glBlendFunc(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);

            gl::glMatrixMode(gl::GL_PROJECTION);
            gl::glPushMatrix();
            gl::glLoadIdentity();
            gl::glOrtho(0.0, w as f64, h as f64, 0.0, -1.0, 1.0);

            gl::glMatrixMode(gl::GL_MODELVIEW);
            gl::glPushMatrix();
            gl::glLoadIdentity();

            gl::glColor4f(0.0, 0.08, 0.025, 0.86);
            draw_screen_rect(0.0, 0.0, w, title);
            draw_screen_rect(0.0, title, border, h - title);
            draw_screen_rect(w - border, title, border, h - title);
            draw_screen_rect(0.0, h - border, w, border);

            gl::glColor4f(0.10, 1.0, 0.30, 0.90);
            draw_screen_rect(0.0, 0.0, w, 1.0);
            draw_screen_rect(0.0, 0.0, 1.0, h);
            draw_screen_rect(w - 1.0, 0.0, 1.0, h);
            draw_screen_rect(0.0, h - 1.0, w, 1.0);

            gl::glColor4f(0.25, 1.0, 0.45, 0.42);
            draw_screen_rect(border, title - 1.0, w - border * 2.0, 1.0);

            let title_scale = 1.0;
            let title_width = measure_client_title_width(WINDOW_TITLE_TEXT, title_scale);
            let title_x = ((w - title_width) * 0.5).max(border + 6.0);
            let title_y = ((title - 7.0 * title_scale) * 0.5).clamp(2.0, title - 8.0);

            gl::glColor4f(0.42, 1.0, 0.55, 0.95);
            draw_client_title_text(WINDOW_TITLE_TEXT, title_x, title_y, title_scale);

            gl::glPopMatrix();
            gl::glMatrixMode(gl::GL_PROJECTION);
            gl::glPopMatrix();
            gl::glMatrixMode(gl::GL_MODELVIEW);
        }
    }

    fn draw_strip(&self, strip: &Strip, polygon_count: &mut u64) {
        for i in 0..GRID_SIZE {
            let glyph = strip.glyphs[i];
            let mut below = strip.spinner_y >= i as f32;
            if strip.erasing {
                below = !below;
            }

            if glyph != 0 && below {
                let brightness = if self.options.do_waves {
                    let phase = (i + (GRID_SIZE - strip.wave_position as usize)) % WAVE_SIZE;
                    let j = (WAVE_SIZE - phase) % WAVE_SIZE;
                    self.brightness_ramp[j]
                } else {
                    1.0
                };

                self.draw_glyph(
                    glyph,
                    strip.highlight[i],
                    strip.x,
                    strip.y - i as f32,
                    strip.z,
                    brightness,
                    polygon_count,
                );
            }
        }

        if !strip.erasing {
            self.draw_glyph(
                strip.spinner_glyph,
                false,
                strip.x,
                strip.y - strip.spinner_y,
                strip.z,
                1.0,
                polygon_count,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_glyph(
        &self,
        glyph: i32,
        highlight: bool,
        mut x: f32,
        mut y: f32,
        z: f32,
        mut brightness: f32,
        polygon_count: &mut u64,
    ) {
        if glyph == 0 {
            return;
        }

        let spinner = glyph < 0;
        let glyph = glyph.abs();
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut size = 1.0;

        if spinner {
            brightness *= 1.5;
        }

        if !self.options.do_texture {
            size = 0.8;
            x += 0.1;
            y += 0.1;
        } else {
            let cell = glyph - 1;
            let ccx = cell % CHAR_COLS as i32;
            let ccy = cell / CHAR_COLS as i32;
            cx = ccx as f32 * self.tex_char_width;
            cy = (self.real_char_rows - ccy - 1) as f32 * self.tex_char_height;

            if self.options.do_fog {
                let mut depth = z / GRID_DEPTH + 0.5;
                depth = 0.2 + depth * 0.8;
                brightness *= depth;
            }
        }

        if highlight {
            brightness *= 2.0;
        }

        let (r, g, b) = if spinner {
            (0.72, 1.0, 0.74)
        } else if highlight {
            (0.38, 1.0, 0.44)
        } else {
            (0.04, 0.92, 0.16)
        };

        let mut alpha = brightness;
        if z > GRID_DEPTH / 2.0 {
            let ratio = (z - GRID_DEPTH / 2.0) / (GRID_DEPTH * SPLASH_RATIO - GRID_DEPTH / 2.0);
            let i = ((ratio * WAVE_SIZE as f32) as usize).min(WAVE_SIZE - 1);
            alpha *= self.brightness_ramp[i];
        }

        unsafe {
            gl::glColor4f(r, g, b, alpha);
            gl::glBegin(if self.options.wireframe {
                gl::GL_LINE_LOOP
            } else {
                gl::GL_QUADS
            });
            gl::glNormal3f(0.0, 0.0, 1.0);
            gl::glTexCoord2f(cx, cy);
            gl::glVertex3f(x, y, z);
            gl::glTexCoord2f(cx + self.tex_char_width, cy);
            gl::glVertex3f(x + size, y, z);
            gl::glTexCoord2f(cx + self.tex_char_width, cy + self.tex_char_height);
            gl::glVertex3f(x + size, y + size, z);
            gl::glTexCoord2f(cx, cy + self.tex_char_height);
            gl::glVertex3f(x, y + size, z);
            gl::glEnd();

            if self.options.wireframe && spinner {
                gl::glBegin(gl::GL_LINES);
                gl::glVertex3f(x, y, z);
                gl::glVertex3f(x + size, y + size, z);
                gl::glVertex3f(x, y + size, z);
                gl::glVertex3f(x + size, y, z);
                gl::glEnd();
            }
        }

        *polygon_count += 1;
    }

    fn auto_track(&mut self) {
        if !self.options.do_rotate || self.button_down {
            return;
        }

        if !self.auto_tracking {
            self.track_tick += 1;
            if self.track_tick < (20.0 / self.options.speed) as i32 {
                return;
            }
            self.track_tick = 0;
            if self.rng.one_in(20) {
                self.auto_tracking = true;
            } else {
                return;
            }
        }

        let origin = NICE_VIEWS[self.last_view];
        let target = NICE_VIEWS[self.target_view];
        let t =
            (std::f32::consts::FRAC_PI_2 * self.view_tick as f32 / self.view_steps as f32).sin();

        self.view_x = origin.x + (target.x - origin.x) * t;
        self.view_y = origin.y + (target.y - origin.y) * t;
        self.view_tick += 1;

        if self.view_tick >= self.view_steps {
            self.view_tick = 0;
            self.view_steps = ((350.0 / self.options.speed) as i32).max(1);
            self.last_view = self.target_view;
            self.target_view = self.rng.usize(NICE_VIEWS.len() - 1) + 1;
            self.auto_tracking = false;
        }
    }
}

impl Drop for Matrix {
    fn drop(&mut self) {
        if self.texture != 0 {
            unsafe {
                gl::glDeleteTextures(1, &self.texture);
            }
        }
    }
}

fn draw_screen_rect(x: f32, y: f32, w: f32, h: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    unsafe {
        gl::glBegin(gl::GL_QUADS);
        gl::glVertex3f(x, y, 0.0);
        gl::glVertex3f(x + w, y, 0.0);
        gl::glVertex3f(x + w, y + h, 0.0);
        gl::glVertex3f(x, y + h, 0.0);
        gl::glEnd();
    }
}

fn draw_client_title_text(text: &str, mut x: f32, y: f32, scale: f32) {
    let char_width = 6.0 * scale;
    let space = 2.0;

    for ch in text.chars() {
        if ch == ' ' {
            x += char_width * 0.5;
            continue;
        }

        if let Some(pattern) = font_pattern(ch) {
            for row in 0..7 {
                for col in 0..5 {
                    if pattern[row] & (1 << (4 - col)) != 0 {
                        draw_screen_rect(
                            x + col as f32 * scale,
                            y + row as f32 * scale,
                            scale,
                            scale,
                        );
                    }
                }
            }
        }

        x += char_width + space;
    }
}

fn measure_client_title_width(text: &str, scale: f32) -> f32 {
    let char_width = 6.0 * scale;
    let space = 2.0;

    let mut width = 0.0;
    for ch in text.chars() {
        width += if ch == ' ' {
            char_width * 0.5
        } else {
            char_width + space
        };
    }

    width.max(0.0)
}

fn fixed_to_f64(value: wayland::WlFixed) -> f64 {
    value as f64 / 256.0
}

fn random_visible_glyph(rng: &mut Rng, glyph_map: &[i32]) -> i32 {
    glyph_map[rng.usize(glyph_map.len())] + 1
}

struct TextureAtlas {
    width: usize,
    height: usize,
    cell: usize,
    real_rows: usize,
    data: Vec<u8>,
}

fn make_texture_atlas(flip: bool) -> TextureAtlas {
    let cell = 32;
    let width = CHAR_COLS * cell;
    let height = 512;
    let mut atlas = TextureAtlas {
        width,
        height,
        cell,
        real_rows: REAL_CHAR_ROWS,
        data: vec![0; width * height * 4],
    };

    for glyph in 0..(CHAR_COLS * REAL_CHAR_ROWS) {
        draw_atlas_glyph(&mut atlas, glyph, flip);
    }

    atlas
}

fn draw_atlas_glyph(atlas: &mut TextureAtlas, glyph: usize, flip: bool) {
    let col = glyph % CHAR_COLS;
    let row = glyph / CHAR_COLS;
    let base_x = col * atlas.cell;
    let base_y = (atlas.real_rows - row - 1) * atlas.cell;

    if let Some(ch) = ascii_char_for_glyph(glyph) {
        if ch == ' ' {
            return;
        }
        if let Some(pattern) = font_pattern(ch) {
            draw_font_pattern(atlas, base_x, base_y, pattern, flip);
            return;
        }
    }

    draw_procedural_symbol(atlas, base_x, base_y, glyph, flip);
}

fn ascii_char_for_glyph(glyph: usize) -> Option<char> {
    if glyph < 96 {
        Some((glyph as u8 + b' ') as char)
    } else {
        None
    }
}

fn draw_font_pattern(
    atlas: &mut TextureAtlas,
    base_x: usize,
    base_y: usize,
    pattern: [u8; 7],
    flip: bool,
) {
    let scale = 4;
    let margin_x = 6;
    let margin_y = 2;
    for (row, bits) in pattern.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                let x = margin_x + col * scale;
                let y = atlas.cell - margin_y - (row + 1) * scale;
                draw_cell_rect(atlas, base_x, base_y, x, y, scale, scale, 230, flip);
                draw_cell_rect(
                    atlas,
                    base_x,
                    base_y,
                    x + 1,
                    y + 1,
                    scale.saturating_sub(2),
                    scale.saturating_sub(2),
                    255,
                    flip,
                );
            }
        }
    }
}

fn draw_procedural_symbol(
    atlas: &mut TextureAtlas,
    base_x: usize,
    base_y: usize,
    glyph: usize,
    flip: bool,
) {
    let mut seed = (glyph as u64 + 1)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .rotate_left((glyph % 31) as u32);
    let scale = 4;
    let margin_x = 6;
    let margin_y = 2;

    for row in 0..7 {
        for col in 0..5 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let edge_bias = col == 0 || col == 4 || row == 0 || row == 6;
            let draw = if edge_bias {
                seed & 0b11 == 0
            } else {
                seed & 0b111 != 0
            };
            if draw {
                let x = margin_x + col * scale;
                let y = atlas.cell - margin_y - (row + 1) * scale;
                let alpha = 170 + (seed as u8 & 0x55);
                draw_cell_rect(atlas, base_x, base_y, x, y, scale, scale, alpha, flip);
            }
        }
    }

    let bar_x = margin_x + ((glyph * 3) % 5) * scale;
    draw_cell_rect(
        atlas,
        base_x,
        base_y,
        bar_x,
        margin_y + 2,
        2,
        atlas.cell - margin_y * 2 - 4,
        190,
        flip,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_cell_rect(
    atlas: &mut TextureAtlas,
    base_x: usize,
    base_y: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    alpha: u8,
    flip: bool,
) {
    if w == 0 || h == 0 {
        return;
    }

    for yy in y..(y + h).min(atlas.cell) {
        for xx in x..(x + w).min(atlas.cell) {
            let local_x = if flip { atlas.cell - xx - 1 } else { xx };
            put_atlas_pixel(atlas, base_x + local_x, base_y + yy, alpha);
        }
    }
}

fn put_atlas_pixel(atlas: &mut TextureAtlas, x: usize, y: usize, alpha: u8) {
    if x >= atlas.width || y >= atlas.height {
        return;
    }
    let index = (y * atlas.width + x) * 4;
    let current = atlas.data[index + 3];
    let alpha = alpha.max(current);
    atlas.data[index] = 255;
    atlas.data[index + 1] = 255;
    atlas.data[index + 2] = 255;
    atlas.data[index + 3] = alpha;
}

fn font_pattern(ch: char) -> Option<[u8; 7]> {
    let pattern = match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10011, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        _ => {
            if ch.is_ascii_lowercase() {
                return font_pattern(ch.to_ascii_uppercase());
            }
            return None;
        }
    };

    Some(pattern)
}

fn current_time_text(format: &str) -> String {
    let Ok(format) = CString::new(format) else {
        return String::new();
    };

    unsafe {
        let mut now: ctime::TimeT = 0;
        ctime::time(&mut now);

        let mut tm = mem::MaybeUninit::<ctime::Tm>::zeroed();
        if ctime::localtime_r(&now, tm.as_mut_ptr()).is_null() {
            return String::new();
        }
        let tm = tm.assume_init();

        let mut buf = [0 as c_char; 128];
        let len = ctime::strftime(buf.as_mut_ptr(), buf.len(), format.as_ptr(), &tm);
        if len == 0 {
            return String::new();
        }

        let bytes = std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), len);
        String::from_utf8_lossy(bytes).into_owned()
    }
}

struct XdgInterfaces {
    wm_base: *const wayland::WlInterface,
    surface: *const wayland::WlInterface,
    toplevel: *const wayland::WlInterface,
    decoration_manager: *const wayland::WlInterface,
    toplevel_decoration: *const wayland::WlInterface,
}

impl XdgInterfaces {
    fn new() -> &'static Self {
        fn c_string(value: &str) -> *const c_char {
            CString::new(value)
                .expect("static string has no NUL")
                .into_raw()
        }

        fn types(values: Vec<*const wayland::WlInterface>) -> *const *const wayland::WlInterface {
            if values.is_empty() {
                ptr::null()
            } else {
                Box::leak(values.into_boxed_slice()).as_ptr()
            }
        }

        fn messages(values: Vec<wayland::WlMessage>) -> *const wayland::WlMessage {
            if values.is_empty() {
                ptr::null()
            } else {
                Box::leak(values.into_boxed_slice()).as_ptr()
            }
        }

        fn message(
            name: &str,
            signature: &str,
            argument_types: Vec<*const wayland::WlInterface>,
        ) -> wayland::WlMessage {
            wayland::WlMessage {
                name: c_string(name),
                signature: c_string(signature),
                types: types(argument_types),
            }
        }

        fn interface(name: &str, version: c_int) -> &'static mut wayland::WlInterface {
            Box::leak(Box::new(wayland::WlInterface {
                name: c_string(name),
                version,
                method_count: 0,
                methods: ptr::null(),
                event_count: 0,
                events: ptr::null(),
            }))
        }

        unsafe {
            let wm_base = interface("xdg_wm_base", 6);
            let surface = interface("xdg_surface", 6);
            let toplevel = interface("xdg_toplevel", 6);
            let positioner = interface("xdg_positioner", 6);
            let popup = interface("xdg_popup", 6);
            let decoration_manager = interface("zxdg_decoration_manager_v1", 1);
            let toplevel_decoration = interface("zxdg_toplevel_decoration_v1", 1);

            positioner.method_count = 10;
            positioner.methods = messages(vec![
                message("destroy", "", vec![]),
                message("set_size", "ii", vec![ptr::null(), ptr::null()]),
                message("set_anchor_rect", "iiii", vec![ptr::null(); 4]),
                message("set_anchor", "u", vec![ptr::null()]),
                message("set_gravity", "u", vec![ptr::null()]),
                message("set_constraint_adjustment", "u", vec![ptr::null()]),
                message("set_offset", "ii", vec![ptr::null(), ptr::null()]),
                message("set_reactive", "3", vec![]),
                message("set_parent_size", "3ii", vec![ptr::null(), ptr::null()]),
                message("set_parent_configure", "3u", vec![ptr::null()]),
            ]);

            popup.method_count = 3;
            popup.methods = messages(vec![
                message("destroy", "", vec![]),
                message("grab", "ou", vec![ptr::null(), ptr::null()]),
                message("reposition", "3ou", vec![positioner, ptr::null()]),
            ]);
            popup.event_count = 3;
            popup.events = messages(vec![
                message("configure", "iiii", vec![ptr::null(); 4]),
                message("popup_done", "", vec![]),
                message("repositioned", "3u", vec![ptr::null()]),
            ]);

            wm_base.method_count = 4;
            wm_base.methods = messages(vec![
                message("destroy", "", vec![]),
                message("create_positioner", "n", vec![positioner]),
                message(
                    "get_xdg_surface",
                    "no",
                    vec![surface, &wayland::wl_surface_interface],
                ),
                message("pong", "u", vec![ptr::null()]),
            ]);
            wm_base.event_count = 1;
            wm_base.events = messages(vec![message("ping", "u", vec![ptr::null()])]);

            surface.method_count = 5;
            surface.methods = messages(vec![
                message("destroy", "", vec![]),
                message("get_toplevel", "n", vec![toplevel]),
                message("get_popup", "n?oo", vec![popup, surface, positioner]),
                message("set_window_geometry", "iiii", vec![ptr::null(); 4]),
                message("ack_configure", "u", vec![ptr::null()]),
            ]);
            surface.event_count = 1;
            surface.events = messages(vec![message("configure", "u", vec![ptr::null()])]);

            toplevel.method_count = 14;
            toplevel.methods = messages(vec![
                message("destroy", "", vec![]),
                message("set_parent", "?o", vec![toplevel]),
                message("set_title", "s", vec![ptr::null()]),
                message("set_app_id", "s", vec![ptr::null()]),
                message("show_window_menu", "ouii", vec![ptr::null(); 4]),
                message("move", "ou", vec![ptr::null(), ptr::null()]),
                message("resize", "ouu", vec![ptr::null(); 3]),
                message("set_max_size", "ii", vec![ptr::null(), ptr::null()]),
                message("set_min_size", "ii", vec![ptr::null(), ptr::null()]),
                message("set_maximized", "", vec![]),
                message("unset_maximized", "", vec![]),
                message("set_fullscreen", "?o", vec![ptr::null()]),
                message("unset_fullscreen", "", vec![]),
                message("set_minimized", "", vec![]),
            ]);
            toplevel.event_count = 4;
            toplevel.events = messages(vec![
                message("configure", "iia", vec![ptr::null(); 3]),
                message("close", "", vec![]),
                message("configure_bounds", "4ii", vec![ptr::null(), ptr::null()]),
                message("wm_capabilities", "5a", vec![ptr::null()]),
            ]);

            decoration_manager.method_count = 2;
            decoration_manager.methods = messages(vec![
                message("destroy", "", vec![]),
                message(
                    "get_toplevel_decoration",
                    "no",
                    vec![toplevel_decoration, toplevel],
                ),
            ]);

            toplevel_decoration.method_count = 3;
            toplevel_decoration.methods = messages(vec![
                message("destroy", "", vec![]),
                message("set_mode", "u", vec![ptr::null()]),
                message("unset_mode", "", vec![]),
            ]);
            toplevel_decoration.event_count = 1;
            toplevel_decoration.events =
                messages(vec![message("configure", "u", vec![ptr::null()])]);

            Box::leak(Box::new(Self {
                wm_base,
                surface,
                toplevel,
                decoration_manager,
                toplevel_decoration,
            }))
        }
    }
}

struct ClientState {
    display: *mut wayland::WlDisplay,
    registry: *mut wayland::WlRegistry,
    compositor: *mut wayland::WlCompositor,
    surface: *mut wayland::WlSurface,
    seat: *mut wayland::WlSeat,
    keyboard: *mut wayland::WlKeyboard,
    pointer: *mut wayland::WlPointer,
    wm_base: *mut wayland::WlProxy,
    xdg_surface: *mut wayland::WlProxy,
    xdg_toplevel: *mut wayland::WlProxy,
    decoration_manager: *mut wayland::WlProxy,
    toplevel_decoration: *mut wayland::WlProxy,
    egl_window: *mut wayland_egl::WlEglWindow,
    xdg: &'static XdgInterfaces,
    configured: bool,
    running: bool,
    pointer_down: bool,
    fullscreen: bool,
    decoration_mode: u32,
    pointer_x: f64,
    pointer_y: f64,
    press_x: f64,
    press_y: f64,
    press_serial: u32,
    press_active: bool,
    last_click_time: Option<u32>,
    last_click_x: f64,
    last_click_y: f64,
    width: u32,
    height: u32,
    pending_width: u32,
    pending_height: u32,
    resized: bool,
}

impl ClientState {
    fn new(display: *mut wayland::WlDisplay, width: u32, height: u32) -> Self {
        Self {
            display,
            registry: ptr::null_mut(),
            compositor: ptr::null_mut(),
            surface: ptr::null_mut(),
            seat: ptr::null_mut(),
            keyboard: ptr::null_mut(),
            pointer: ptr::null_mut(),
            wm_base: ptr::null_mut(),
            xdg_surface: ptr::null_mut(),
            xdg_toplevel: ptr::null_mut(),
            decoration_manager: ptr::null_mut(),
            toplevel_decoration: ptr::null_mut(),
            egl_window: ptr::null_mut(),
            xdg: XdgInterfaces::new(),
            configured: false,
            running: true,
            pointer_down: false,
            fullscreen: false,
            decoration_mode: ZXDG_TOPLEVEL_DECORATION_V1_MODE_CLIENT_SIDE,
            pointer_x: 0.0,
            pointer_y: 0.0,
            press_x: 0.0,
            press_y: 0.0,
            press_serial: 0,
            press_active: false,
            last_click_time: None,
            last_click_x: 0.0,
            last_click_y: 0.0,
            width,
            height,
            pending_width: width,
            pending_height: height,
            resized: false,
        }
    }

    fn uses_client_decoration(&self) -> bool {
        !self.fullscreen && self.decoration_mode != ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE
    }

    fn update_pointer(&mut self, x: wayland::WlFixed, y: wayland::WlFixed) {
        self.pointer_x = fixed_to_f64(x);
        self.pointer_y = fixed_to_f64(y);
    }

    fn resize_edge_at_pointer(&self) -> u32 {
        if self.fullscreen {
            return XDG_TOPLEVEL_RESIZE_EDGE_NONE;
        }

        let width = self.width as f64;
        let height = self.height as f64;
        let left = self.pointer_x <= RESIZE_GRAB_MARGIN;
        let right = self.pointer_x >= width - RESIZE_GRAB_MARGIN;
        let top = self.pointer_y <= RESIZE_GRAB_MARGIN;
        let bottom = self.pointer_y >= height - RESIZE_GRAB_MARGIN;

        let mut edge = XDG_TOPLEVEL_RESIZE_EDGE_NONE;
        if top {
            edge |= XDG_TOPLEVEL_RESIZE_EDGE_TOP;
        }
        if bottom {
            edge |= XDG_TOPLEVEL_RESIZE_EDGE_BOTTOM;
        }
        if left {
            edge |= XDG_TOPLEVEL_RESIZE_EDGE_LEFT;
        }
        if right {
            edge |= XDG_TOPLEVEL_RESIZE_EDGE_RIGHT;
        }
        edge
    }

    fn is_double_click(&self, time: u32) -> bool {
        let Some(last_time) = self.last_click_time else {
            return false;
        };

        let elapsed = time.wrapping_sub(last_time);
        let dx = self.pointer_x - self.last_click_x;
        let dy = self.pointer_y - self.last_click_y;
        elapsed <= DOUBLE_CLICK_MS && (dx * dx + dy * dy).sqrt() <= DOUBLE_CLICK_DISTANCE
    }

    unsafe fn toggle_fullscreen(&mut self) {
        if self.xdg_toplevel.is_null() {
            return;
        }

        if self.fullscreen {
            xdg_toplevel_unset_fullscreen(self.xdg_toplevel);
            self.fullscreen = false;
        } else {
            xdg_toplevel_set_fullscreen(self.xdg_toplevel);
            self.fullscreen = true;
        }
    }

    unsafe fn start_interactive_resize(&mut self, serial: u32, edge: u32) {
        if self.xdg_toplevel.is_null()
            || self.seat.is_null()
            || edge == XDG_TOPLEVEL_RESIZE_EDGE_NONE
        {
            return;
        }

        xdg_toplevel_resize(self.xdg_toplevel, self.seat, serial, edge);
    }

    unsafe fn maybe_start_interactive_move(&mut self) {
        if self.fullscreen || self.xdg_toplevel.is_null() || self.seat.is_null() {
            return;
        }

        let dx = self.pointer_x - self.press_x;
        let dy = self.pointer_y - self.press_y;
        if (dx * dx + dy * dy).sqrt() < MOVE_DRAG_THRESHOLD {
            return;
        }

        xdg_toplevel_move(self.xdg_toplevel, self.seat, self.press_serial);
        self.pointer_down = false;
        self.press_active = false;
    }

    fn apply_configure_size(&mut self) {
        if self.pending_width == 0 || self.pending_height == 0 {
            return;
        }

        if self.pending_width == self.width && self.pending_height == self.height {
            return;
        }

        self.width = self.pending_width;
        self.height = self.pending_height;
        self.resized = true;

        if !self.egl_window.is_null() {
            unsafe {
                wayland_egl::wl_egl_window_resize(
                    self.egl_window,
                    self.width as c_int,
                    self.height as c_int,
                    0,
                    0,
                );
            }
        }
    }
}

struct WaylandWindow {
    state: Box<ClientState>,
    egl_display: egl::EGLDisplay,
    egl_surface: egl::EGLSurface,
    egl_context: egl::EGLContext,
}

impl WaylandWindow {
    fn new(width: u32, height: u32, title: &str) -> Result<Self, String> {
        unsafe {
            let display = wayland::wl_display_connect(ptr::null());
            if display.is_null() {
                return Err("could not connect to a Wayland compositor".to_string());
            }

            let mut state = Box::new(ClientState::new(display, width, height));
            let state_ptr = state.as_mut() as *mut ClientState;

            state.registry = display_get_registry(display);
            if state.registry.is_null() {
                wayland::wl_display_disconnect(display);
                return Err("could not get the Wayland registry".to_string());
            }

            wayland::wl_proxy_add_listener(
                state.registry.cast(),
                (&REGISTRY_LISTENER as *const wayland::WlRegistryListener).cast(),
                state_ptr.cast(),
            );

            if wayland::wl_display_roundtrip(display) < 0 {
                wayland::wl_display_disconnect(display);
                return Err("Wayland registry roundtrip failed".to_string());
            }

            if state.compositor.is_null() {
                wayland::wl_display_disconnect(display);
                return Err("Wayland compositor does not expose wl_compositor".to_string());
            }

            if state.wm_base.is_null() {
                wayland::wl_display_disconnect(display);
                return Err("Wayland compositor does not expose xdg_wm_base".to_string());
            }

            state.surface = compositor_create_surface(state.compositor);
            if state.surface.is_null() {
                wayland::wl_display_disconnect(display);
                return Err("could not create a Wayland surface".to_string());
            }

            state.xdg_surface =
                xdg_wm_base_get_xdg_surface(state.wm_base, state.xdg.surface, state.surface);
            if state.xdg_surface.is_null() {
                wayland::wl_display_disconnect(display);
                return Err("could not create an xdg_surface".to_string());
            }

            wayland::wl_proxy_add_listener(
                state.xdg_surface,
                (&XDG_SURFACE_LISTENER as *const XdgSurfaceListener).cast(),
                state_ptr.cast(),
            );

            state.xdg_toplevel = xdg_surface_get_toplevel(state.xdg_surface, state.xdg.toplevel);
            if state.xdg_toplevel.is_null() {
                wayland::wl_display_disconnect(display);
                return Err("could not create an xdg_toplevel".to_string());
            }

            wayland::wl_proxy_add_listener(
                state.xdg_toplevel,
                (&XDG_TOPLEVEL_LISTENER as *const XdgToplevelListener).cast(),
                state_ptr.cast(),
            );

            if !state.decoration_manager.is_null() {
                state.toplevel_decoration = decoration_manager_get_toplevel_decoration(
                    state.decoration_manager,
                    state.xdg.toplevel_decoration,
                    state.xdg_toplevel,
                );
                if !state.toplevel_decoration.is_null() {
                    wayland::wl_proxy_add_listener(
                        state.toplevel_decoration,
                        (&TOPLEVEL_DECORATION_LISTENER as *const ToplevelDecorationListener).cast(),
                        state_ptr.cast(),
                    );
                    toplevel_decoration_set_mode(
                        state.toplevel_decoration,
                        ZXDG_TOPLEVEL_DECORATION_V1_MODE_CLIENT_SIDE,
                    );
                }
            }

            let title = CString::new(title).map_err(|_| "window title contains NUL")?;
            xdg_toplevel_set_title(state.xdg_toplevel, title.as_ptr());
            let app_id = CString::new("glmatrix-rs").unwrap();
            xdg_toplevel_set_app_id(state.xdg_toplevel, app_id.as_ptr());
            surface_commit(state.surface);

            while !(*state_ptr).configured {
                if wayland::wl_display_dispatch(display) < 0 {
                    wayland::wl_display_disconnect(display);
                    return Err("Wayland dispatch failed while waiting for configure".to_string());
                }
            }

            let egl_display = egl::eglGetDisplay(display.cast());
            if egl_display.is_null() {
                wayland::wl_display_disconnect(display);
                return Err("eglGetDisplay failed".to_string());
            }

            let mut major = 0;
            let mut minor = 0;
            if egl::eglInitialize(egl_display, &mut major, &mut minor) == egl::EGL_FALSE {
                wayland::wl_display_disconnect(display);
                return Err(format!("eglInitialize failed: 0x{:x}", egl::eglGetError()));
            }

            if egl::eglBindAPI(egl::EGL_OPENGL_API) == egl::EGL_FALSE {
                egl::eglTerminate(egl_display);
                wayland::wl_display_disconnect(display);
                return Err(format!(
                    "eglBindAPI(EGL_OPENGL_API) failed: 0x{:x}",
                    egl::eglGetError()
                ));
            }

            let config = choose_egl_config(egl_display)?;
            let context = create_egl_context(egl_display, config)?;

            state.egl_window = wayland_egl::wl_egl_window_create(
                state.surface,
                state.width as c_int,
                state.height as c_int,
            );
            if state.egl_window.is_null() {
                egl::eglDestroyContext(egl_display, context);
                egl::eglTerminate(egl_display);
                wayland::wl_display_disconnect(display);
                return Err("wl_egl_window_create failed".to_string());
            }

            let egl_surface = egl::eglCreateWindowSurface(
                egl_display,
                config,
                state.egl_window.cast(),
                ptr::null(),
            );
            if egl_surface.is_null() {
                wayland_egl::wl_egl_window_destroy(state.egl_window);
                egl::eglDestroyContext(egl_display, context);
                egl::eglTerminate(egl_display);
                wayland::wl_display_disconnect(display);
                return Err(format!(
                    "eglCreateWindowSurface failed: 0x{:x}",
                    egl::eglGetError()
                ));
            }

            if egl::eglMakeCurrent(egl_display, egl_surface, egl_surface, context) == egl::EGL_FALSE
            {
                egl::eglDestroySurface(egl_display, egl_surface);
                wayland_egl::wl_egl_window_destroy(state.egl_window);
                egl::eglDestroyContext(egl_display, context);
                egl::eglTerminate(egl_display);
                wayland::wl_display_disconnect(display);
                return Err(format!("eglMakeCurrent failed: 0x{:x}", egl::eglGetError()));
            }

            egl::eglSwapInterval(egl_display, 1);

            Ok(Self {
                state,
                egl_display,
                egl_surface,
                egl_context: context,
            })
        }
    }

    fn size(&self) -> (u32, u32) {
        (self.state.width, self.state.height)
    }

    fn uses_client_decoration(&self) -> bool {
        self.state.uses_client_decoration()
    }

    fn poll_events(&mut self, matrix: &mut Matrix) -> bool {
        unsafe {
            while wayland::wl_display_dispatch_pending(self.state.display) > 0 {}
            self.apply_state(matrix);

            wayland::wl_display_flush(self.state.display);
            if wayland::wl_display_prepare_read(self.state.display) == 0 {
                wayland::wl_display_flush(self.state.display);
                let fd = wayland::wl_display_get_fd(self.state.display);
                let mut poll_fd = PollFd {
                    fd,
                    events: POLLIN,
                    revents: 0,
                };

                let ready = poll(&mut poll_fd, 1, 0);
                if ready > 0 && (poll_fd.revents & POLLIN) != 0 {
                    wayland::wl_display_read_events(self.state.display);
                } else {
                    wayland::wl_display_cancel_read(self.state.display);
                }
            }

            while wayland::wl_display_dispatch_pending(self.state.display) > 0 {}
            self.apply_state(matrix);

            self.state.running && wayland::wl_display_get_error(self.state.display) == 0
        }
    }

    fn apply_state(&mut self, matrix: &mut Matrix) {
        matrix.set_button_down(self.state.pointer_down);
        if self.state.resized {
            self.state.resized = false;
            matrix.reshape(self.state.width, self.state.height);
        }
    }

    fn swap_buffers(&self) {
        unsafe {
            egl::eglSwapBuffers(self.egl_display, self.egl_surface);
        }
    }
}

impl Drop for WaylandWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.egl_display.is_null() {
                egl::eglMakeCurrent(
                    self.egl_display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                if !self.egl_surface.is_null() {
                    egl::eglDestroySurface(self.egl_display, self.egl_surface);
                }
                if !self.egl_context.is_null() {
                    egl::eglDestroyContext(self.egl_display, self.egl_context);
                }
                egl::eglTerminate(self.egl_display);
            }

            if !self.state.egl_window.is_null() {
                wayland_egl::wl_egl_window_destroy(self.state.egl_window);
            }

            if !self.state.keyboard.is_null() {
                wayland::wl_proxy_destroy(self.state.keyboard.cast());
            }
            if !self.state.pointer.is_null() {
                wayland::wl_proxy_destroy(self.state.pointer.cast());
            }
            if !self.state.seat.is_null() {
                wayland::wl_proxy_destroy(self.state.seat.cast());
            }
            if !self.state.toplevel_decoration.is_null() {
                wayland::wl_proxy_destroy(self.state.toplevel_decoration);
            }
            if !self.state.decoration_manager.is_null() {
                wayland::wl_proxy_destroy(self.state.decoration_manager);
            }
            if !self.state.xdg_toplevel.is_null() {
                wayland::wl_proxy_destroy(self.state.xdg_toplevel);
            }
            if !self.state.xdg_surface.is_null() {
                wayland::wl_proxy_destroy(self.state.xdg_surface);
            }
            if !self.state.surface.is_null() {
                wayland::wl_proxy_destroy(self.state.surface.cast());
            }
            if !self.state.wm_base.is_null() {
                wayland::wl_proxy_destroy(self.state.wm_base);
            }
            if !self.state.compositor.is_null() {
                wayland::wl_proxy_destroy(self.state.compositor.cast());
            }
            if !self.state.registry.is_null() {
                wayland::wl_proxy_destroy(self.state.registry.cast());
            }
            if !self.state.display.is_null() {
                wayland::wl_display_disconnect(self.state.display);
            }
        }
    }
}

unsafe fn choose_egl_config(display: egl::EGLDisplay) -> Result<egl::EGLConfig, String> {
    let attributes = [
        egl::EGL_SURFACE_TYPE,
        egl::EGL_WINDOW_BIT,
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
        egl::EGL_DEPTH_SIZE,
        16,
        egl::EGL_NONE,
    ];
    let mut config = ptr::null_mut();
    let mut count = 0;

    if egl::eglChooseConfig(display, attributes.as_ptr(), &mut config, 1, &mut count)
        == egl::EGL_FALSE
        || count == 0
    {
        return Err(format!(
            "eglChooseConfig failed: 0x{:x}",
            egl::eglGetError()
        ));
    }

    Ok(config)
}

unsafe fn create_egl_context(
    display: egl::EGLDisplay,
    config: egl::EGLConfig,
) -> Result<egl::EGLContext, String> {
    let attributes = [
        egl::EGL_CONTEXT_MAJOR_VERSION,
        2,
        egl::EGL_CONTEXT_MINOR_VERSION,
        1,
        egl::EGL_NONE,
    ];
    let mut context = egl::eglCreateContext(display, config, ptr::null_mut(), attributes.as_ptr());

    if context.is_null() {
        let fallback = [egl::EGL_NONE];
        context = egl::eglCreateContext(display, config, ptr::null_mut(), fallback.as_ptr());
    }

    if context.is_null() {
        return Err(format!(
            "eglCreateContext failed: 0x{:x}",
            egl::eglGetError()
        ));
    }

    Ok(context)
}

unsafe fn display_get_registry(display: *mut wayland::WlDisplay) -> *mut wayland::WlRegistry {
    wayland::wl_proxy_marshal_flags(
        display.cast(),
        wayland::WL_DISPLAY_GET_REGISTRY,
        &wayland::wl_registry_interface,
        wayland::wl_proxy_get_version(display.cast()),
        0,
        ptr::null_mut::<c_void>(),
    )
    .cast()
}

unsafe fn registry_bind(
    registry: *mut wayland::WlRegistry,
    name: u32,
    interface: *const wayland::WlInterface,
    version: u32,
) -> *mut wayland::WlProxy {
    wayland::wl_proxy_marshal_flags(
        registry.cast(),
        wayland::WL_REGISTRY_BIND,
        interface,
        version,
        0,
        name,
        (*interface).name,
        version,
        ptr::null_mut::<c_void>(),
    )
}

unsafe fn compositor_create_surface(
    compositor: *mut wayland::WlCompositor,
) -> *mut wayland::WlSurface {
    wayland::wl_proxy_marshal_flags(
        compositor.cast(),
        wayland::WL_COMPOSITOR_CREATE_SURFACE,
        &wayland::wl_surface_interface,
        wayland::wl_proxy_get_version(compositor.cast()),
        0,
        ptr::null_mut::<c_void>(),
    )
    .cast()
}

unsafe fn surface_commit(surface: *mut wayland::WlSurface) {
    wayland::wl_proxy_marshal_flags(
        surface.cast(),
        wayland::WL_SURFACE_COMMIT,
        ptr::null(),
        wayland::wl_proxy_get_version(surface.cast()),
        0,
    );
}

unsafe fn xdg_wm_base_get_xdg_surface(
    wm_base: *mut wayland::WlProxy,
    xdg_surface_interface: *const wayland::WlInterface,
    surface: *mut wayland::WlSurface,
) -> *mut wayland::WlProxy {
    wayland::wl_proxy_marshal_flags(
        wm_base,
        XDG_WM_BASE_GET_XDG_SURFACE,
        xdg_surface_interface,
        wayland::wl_proxy_get_version(wm_base),
        0,
        ptr::null_mut::<c_void>(),
        surface,
    )
}

unsafe fn xdg_wm_base_pong(wm_base: *mut wayland::WlProxy, serial: u32) {
    wayland::wl_proxy_marshal_flags(
        wm_base,
        XDG_WM_BASE_PONG,
        ptr::null(),
        wayland::wl_proxy_get_version(wm_base),
        0,
        serial,
    );
}

unsafe fn xdg_surface_get_toplevel(
    xdg_surface: *mut wayland::WlProxy,
    xdg_toplevel_interface: *const wayland::WlInterface,
) -> *mut wayland::WlProxy {
    wayland::wl_proxy_marshal_flags(
        xdg_surface,
        XDG_SURFACE_GET_TOPLEVEL,
        xdg_toplevel_interface,
        wayland::wl_proxy_get_version(xdg_surface),
        0,
        ptr::null_mut::<c_void>(),
    )
}

unsafe fn xdg_surface_ack_configure(xdg_surface: *mut wayland::WlProxy, serial: u32) {
    wayland::wl_proxy_marshal_flags(
        xdg_surface,
        XDG_SURFACE_ACK_CONFIGURE,
        ptr::null(),
        wayland::wl_proxy_get_version(xdg_surface),
        0,
        serial,
    );
}

unsafe fn xdg_toplevel_set_title(toplevel: *mut wayland::WlProxy, title: *const c_char) {
    wayland::wl_proxy_marshal_flags(
        toplevel,
        XDG_TOPLEVEL_SET_TITLE,
        ptr::null(),
        wayland::wl_proxy_get_version(toplevel),
        0,
        title,
    );
}

unsafe fn xdg_toplevel_set_app_id(toplevel: *mut wayland::WlProxy, app_id: *const c_char) {
    wayland::wl_proxy_marshal_flags(
        toplevel,
        XDG_TOPLEVEL_SET_APP_ID,
        ptr::null(),
        wayland::wl_proxy_get_version(toplevel),
        0,
        app_id,
    );
}

unsafe fn xdg_toplevel_move(
    toplevel: *mut wayland::WlProxy,
    seat: *mut wayland::WlSeat,
    serial: u32,
) {
    wayland::wl_proxy_marshal_flags(
        toplevel,
        XDG_TOPLEVEL_MOVE,
        ptr::null(),
        wayland::wl_proxy_get_version(toplevel),
        0,
        seat,
        serial,
    );
}

unsafe fn xdg_toplevel_resize(
    toplevel: *mut wayland::WlProxy,
    seat: *mut wayland::WlSeat,
    serial: u32,
    edges: u32,
) {
    wayland::wl_proxy_marshal_flags(
        toplevel,
        XDG_TOPLEVEL_RESIZE,
        ptr::null(),
        wayland::wl_proxy_get_version(toplevel),
        0,
        seat,
        serial,
        edges,
    );
}

unsafe fn xdg_toplevel_set_fullscreen(toplevel: *mut wayland::WlProxy) {
    wayland::wl_proxy_marshal_flags(
        toplevel,
        XDG_TOPLEVEL_SET_FULLSCREEN,
        ptr::null(),
        wayland::wl_proxy_get_version(toplevel),
        0,
        ptr::null_mut::<c_void>(),
    );
}

unsafe fn xdg_toplevel_unset_fullscreen(toplevel: *mut wayland::WlProxy) {
    wayland::wl_proxy_marshal_flags(
        toplevel,
        XDG_TOPLEVEL_UNSET_FULLSCREEN,
        ptr::null(),
        wayland::wl_proxy_get_version(toplevel),
        0,
    );
}

unsafe fn decoration_manager_get_toplevel_decoration(
    manager: *mut wayland::WlProxy,
    decoration_interface: *const wayland::WlInterface,
    toplevel: *mut wayland::WlProxy,
) -> *mut wayland::WlProxy {
    wayland::wl_proxy_marshal_flags(
        manager,
        ZXDG_DECORATION_MANAGER_V1_GET_TOPLEVEL_DECORATION,
        decoration_interface,
        wayland::wl_proxy_get_version(manager),
        0,
        ptr::null_mut::<c_void>(),
        toplevel,
    )
}

unsafe fn toplevel_decoration_set_mode(decoration: *mut wayland::WlProxy, mode: u32) {
    wayland::wl_proxy_marshal_flags(
        decoration,
        ZXDG_TOPLEVEL_DECORATION_V1_SET_MODE,
        ptr::null(),
        wayland::wl_proxy_get_version(decoration),
        0,
        mode,
    );
}

unsafe fn seat_get_keyboard(seat: *mut wayland::WlSeat) -> *mut wayland::WlKeyboard {
    wayland::wl_proxy_marshal_flags(
        seat.cast(),
        wayland::WL_SEAT_GET_KEYBOARD,
        &wayland::wl_keyboard_interface,
        wayland::wl_proxy_get_version(seat.cast()),
        0,
        ptr::null_mut::<c_void>(),
    )
    .cast()
}

unsafe fn seat_get_pointer(seat: *mut wayland::WlSeat) -> *mut wayland::WlPointer {
    wayland::wl_proxy_marshal_flags(
        seat.cast(),
        wayland::WL_SEAT_GET_POINTER,
        &wayland::wl_pointer_interface,
        wayland::wl_proxy_get_version(seat.cast()),
        0,
        ptr::null_mut::<c_void>(),
    )
    .cast()
}

const XDG_WM_BASE_GET_XDG_SURFACE: u32 = 2;
const XDG_WM_BASE_PONG: u32 = 3;
const XDG_SURFACE_GET_TOPLEVEL: u32 = 1;
const XDG_SURFACE_ACK_CONFIGURE: u32 = 4;
const XDG_TOPLEVEL_SET_TITLE: u32 = 2;
const XDG_TOPLEVEL_SET_APP_ID: u32 = 3;
const XDG_TOPLEVEL_MOVE: u32 = 5;
const XDG_TOPLEVEL_RESIZE: u32 = 6;
const XDG_TOPLEVEL_SET_FULLSCREEN: u32 = 11;
const XDG_TOPLEVEL_UNSET_FULLSCREEN: u32 = 12;
const XDG_TOPLEVEL_RESIZE_EDGE_NONE: u32 = 0;
const XDG_TOPLEVEL_RESIZE_EDGE_TOP: u32 = 1;
const XDG_TOPLEVEL_RESIZE_EDGE_BOTTOM: u32 = 2;
const XDG_TOPLEVEL_RESIZE_EDGE_LEFT: u32 = 4;
const XDG_TOPLEVEL_RESIZE_EDGE_RIGHT: u32 = 8;
const XDG_TOPLEVEL_STATE_FULLSCREEN: u32 = 2;
const ZXDG_DECORATION_MANAGER_V1_GET_TOPLEVEL_DECORATION: u32 = 1;
const ZXDG_TOPLEVEL_DECORATION_V1_SET_MODE: u32 = 1;
const ZXDG_TOPLEVEL_DECORATION_V1_MODE_CLIENT_SIDE: u32 = 1;
const ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE: u32 = 2;
const WL_SEAT_CAPABILITY_POINTER: u32 = 1;
const WL_SEAT_CAPABILITY_KEYBOARD: u32 = 2;
const WL_KEYBOARD_KEY_STATE_PRESSED: u32 = 1;
const WL_POINTER_BUTTON_STATE_PRESSED: u32 = 1;
const WL_POINTER_BUTTON_STATE_RELEASED: u32 = 0;
const KEY_ESC: u32 = 1;
const KEY_Q: u32 = 16;
const KEY_F: u32 = 33;
const BTN_LEFT: u32 = 0x110;

static REGISTRY_LISTENER: wayland::WlRegistryListener = wayland::WlRegistryListener {
    global: Some(registry_global),
    global_remove: Some(registry_global_remove),
};

static XDG_WM_BASE_LISTENER: XdgWmBaseListener = XdgWmBaseListener {
    ping: Some(xdg_wm_base_ping),
};

static XDG_SURFACE_LISTENER: XdgSurfaceListener = XdgSurfaceListener {
    configure: Some(xdg_surface_configure),
};

static XDG_TOPLEVEL_LISTENER: XdgToplevelListener = XdgToplevelListener {
    configure: Some(xdg_toplevel_configure),
    close: Some(xdg_toplevel_close),
    configure_bounds: Some(xdg_toplevel_configure_bounds),
    wm_capabilities: Some(xdg_toplevel_wm_capabilities),
};

static TOPLEVEL_DECORATION_LISTENER: ToplevelDecorationListener = ToplevelDecorationListener {
    configure: Some(toplevel_decoration_configure),
};

static SEAT_LISTENER: wayland::WlSeatListener = wayland::WlSeatListener {
    capabilities: Some(seat_capabilities),
    name: Some(seat_name),
};

static KEYBOARD_LISTENER: wayland::WlKeyboardListener = wayland::WlKeyboardListener {
    keymap: Some(keyboard_keymap),
    enter: Some(keyboard_enter),
    leave: Some(keyboard_leave),
    key: Some(keyboard_key),
    modifiers: Some(keyboard_modifiers),
    repeat_info: Some(keyboard_repeat_info),
};

static POINTER_LISTENER: wayland::WlPointerListener = wayland::WlPointerListener {
    enter: Some(pointer_enter),
    leave: Some(pointer_leave),
    motion: Some(pointer_motion),
    button: Some(pointer_button),
    axis: Some(pointer_axis),
    frame: Some(pointer_frame),
    axis_source: Some(pointer_axis_source),
    axis_stop: Some(pointer_axis_stop),
    axis_discrete: Some(pointer_axis_discrete),
    axis_value120: Some(pointer_axis_value120),
    axis_relative_direction: Some(pointer_axis_relative_direction),
};

unsafe extern "C" fn registry_global(
    data: *mut c_void,
    registry: *mut wayland::WlRegistry,
    name: u32,
    interface: *const c_char,
    version: u32,
) {
    let state = &mut *(data.cast::<ClientState>());
    let interface = CStr::from_ptr(interface).to_string_lossy();

    if interface == "wl_compositor" {
        let version = version.min(4);
        state.compositor =
            registry_bind(registry, name, &wayland::wl_compositor_interface, version).cast();
    } else if interface == "xdg_wm_base" {
        let version = version.min(6);
        state.wm_base = registry_bind(registry, name, state.xdg.wm_base, version);
        wayland::wl_proxy_add_listener(
            state.wm_base,
            (&XDG_WM_BASE_LISTENER as *const XdgWmBaseListener).cast(),
            data,
        );
    } else if interface == "wl_seat" {
        let version = version.min(5);
        state.seat = registry_bind(registry, name, &wayland::wl_seat_interface, version).cast();
        wayland::wl_proxy_add_listener(
            state.seat.cast(),
            (&SEAT_LISTENER as *const wayland::WlSeatListener).cast(),
            data,
        );
    } else if interface == "zxdg_decoration_manager_v1" {
        state.decoration_manager =
            registry_bind(registry, name, state.xdg.decoration_manager, version.min(1));
    }
}

unsafe extern "C" fn registry_global_remove(
    _data: *mut c_void,
    _registry: *mut wayland::WlRegistry,
    _name: u32,
) {
}

unsafe extern "C" fn xdg_wm_base_ping(
    _data: *mut c_void,
    wm_base: *mut wayland::WlProxy,
    serial: u32,
) {
    xdg_wm_base_pong(wm_base, serial);
}

unsafe extern "C" fn xdg_surface_configure(
    data: *mut c_void,
    xdg_surface: *mut wayland::WlProxy,
    serial: u32,
) {
    let state = &mut *(data.cast::<ClientState>());
    xdg_surface_ack_configure(xdg_surface, serial);
    state.apply_configure_size();
    state.configured = true;
    if !state.surface.is_null() {
        surface_commit(state.surface);
    }
}

unsafe extern "C" fn xdg_toplevel_configure(
    data: *mut c_void,
    _toplevel: *mut wayland::WlProxy,
    width: c_int,
    height: c_int,
    states: *mut wayland::WlArray,
) {
    let state = &mut *(data.cast::<ClientState>());
    if width > 0 && height > 0 {
        state.pending_width = width as u32;
        state.pending_height = height as u32;
    }

    if !states.is_null() && !(*states).data.is_null() {
        let len = (*states).size / mem::size_of::<u32>();
        let values = std::slice::from_raw_parts((*states).data.cast::<u32>(), len);
        state.fullscreen = values.contains(&XDG_TOPLEVEL_STATE_FULLSCREEN);
    }
}

unsafe extern "C" fn xdg_toplevel_close(data: *mut c_void, _toplevel: *mut wayland::WlProxy) {
    (*(data.cast::<ClientState>())).running = false;
}

unsafe extern "C" fn xdg_toplevel_configure_bounds(
    _data: *mut c_void,
    _toplevel: *mut wayland::WlProxy,
    _width: c_int,
    _height: c_int,
) {
}

unsafe extern "C" fn xdg_toplevel_wm_capabilities(
    _data: *mut c_void,
    _toplevel: *mut wayland::WlProxy,
    _capabilities: *mut wayland::WlArray,
) {
}

unsafe extern "C" fn toplevel_decoration_configure(
    data: *mut c_void,
    _decoration: *mut wayland::WlProxy,
    mode: u32,
) {
    (*(data.cast::<ClientState>())).decoration_mode = mode;
}

unsafe extern "C" fn seat_capabilities(
    data: *mut c_void,
    seat: *mut wayland::WlSeat,
    capabilities: u32,
) {
    let state = &mut *(data.cast::<ClientState>());

    if (capabilities & WL_SEAT_CAPABILITY_KEYBOARD) != 0 && state.keyboard.is_null() {
        state.keyboard = seat_get_keyboard(seat);
        if !state.keyboard.is_null() {
            wayland::wl_proxy_add_listener(
                state.keyboard.cast(),
                (&KEYBOARD_LISTENER as *const wayland::WlKeyboardListener).cast(),
                data,
            );
        }
    }

    if (capabilities & WL_SEAT_CAPABILITY_POINTER) != 0 && state.pointer.is_null() {
        state.pointer = seat_get_pointer(seat);
        if !state.pointer.is_null() {
            wayland::wl_proxy_add_listener(
                state.pointer.cast(),
                (&POINTER_LISTENER as *const wayland::WlPointerListener).cast(),
                data,
            );
        }
    }
}

unsafe extern "C" fn seat_name(
    _data: *mut c_void,
    _seat: *mut wayland::WlSeat,
    _name: *const c_char,
) {
}

unsafe extern "C" fn keyboard_keymap(
    _data: *mut c_void,
    _keyboard: *mut wayland::WlKeyboard,
    _format: u32,
    fd: c_int,
    _size: u32,
) {
    close(fd);
}

unsafe extern "C" fn keyboard_enter(
    _data: *mut c_void,
    _keyboard: *mut wayland::WlKeyboard,
    _serial: u32,
    _surface: *mut wayland::WlSurface,
    _keys: *mut wayland::WlArray,
) {
}

unsafe extern "C" fn keyboard_leave(
    _data: *mut c_void,
    _keyboard: *mut wayland::WlKeyboard,
    _serial: u32,
    _surface: *mut wayland::WlSurface,
) {
}

unsafe extern "C" fn keyboard_key(
    data: *mut c_void,
    _keyboard: *mut wayland::WlKeyboard,
    _serial: u32,
    _time: u32,
    key: u32,
    state_value: u32,
) {
    if state_value == WL_KEYBOARD_KEY_STATE_PRESSED {
        let state = &mut *(data.cast::<ClientState>());
        if key == KEY_ESC || key == KEY_Q {
            state.running = false;
            return;
        }
        if key == KEY_F {
            state.toggle_fullscreen();
        }
    }
}

unsafe extern "C" fn keyboard_modifiers(
    _data: *mut c_void,
    _keyboard: *mut wayland::WlKeyboard,
    _serial: u32,
    _mods_depressed: u32,
    _mods_latched: u32,
    _mods_locked: u32,
    _group: u32,
) {
}

unsafe extern "C" fn keyboard_repeat_info(
    _data: *mut c_void,
    _keyboard: *mut wayland::WlKeyboard,
    _rate: c_int,
    _delay: c_int,
) {
}

unsafe extern "C" fn pointer_enter(
    data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _serial: u32,
    _surface: *mut wayland::WlSurface,
    surface_x: wayland::WlFixed,
    surface_y: wayland::WlFixed,
) {
    (*(data.cast::<ClientState>())).update_pointer(surface_x, surface_y);
}

unsafe extern "C" fn pointer_leave(
    data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _serial: u32,
    _surface: *mut wayland::WlSurface,
) {
    let state = &mut *(data.cast::<ClientState>());
    state.pointer_down = false;
    state.press_active = false;
}

unsafe extern "C" fn pointer_motion(
    data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _time: u32,
    surface_x: wayland::WlFixed,
    surface_y: wayland::WlFixed,
) {
    let state = &mut *(data.cast::<ClientState>());
    state.update_pointer(surface_x, surface_y);
    if state.pointer_down && state.press_active {
        state.maybe_start_interactive_move();
    }
}

unsafe extern "C" fn pointer_button(
    data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    serial: u32,
    time: u32,
    button: u32,
    state_value: u32,
) {
    if button == BTN_LEFT {
        let state = &mut *(data.cast::<ClientState>());
        if state_value == WL_POINTER_BUTTON_STATE_PRESSED {
            if state.is_double_click(time) {
                state.pointer_down = false;
                state.press_active = false;
                state.last_click_time = None;
                state.toggle_fullscreen();
                return;
            }

            let edge = state.resize_edge_at_pointer();
            if edge != XDG_TOPLEVEL_RESIZE_EDGE_NONE {
                state.pointer_down = false;
                state.press_active = false;
                state.start_interactive_resize(serial, edge);
                return;
            }

            state.pointer_down = true;
            state.press_active = true;
            state.press_serial = serial;
            state.press_x = state.pointer_x;
            state.press_y = state.pointer_y;
            state.last_click_time = Some(time);
            state.last_click_x = state.pointer_x;
            state.last_click_y = state.pointer_y;
        } else if state_value == WL_POINTER_BUTTON_STATE_RELEASED {
            state.pointer_down = false;
            state.press_active = false;
        }
    }
}

unsafe extern "C" fn pointer_axis(
    _data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _time: u32,
    _axis: u32,
    _value: wayland::WlFixed,
) {
}

unsafe extern "C" fn pointer_frame(_data: *mut c_void, _pointer: *mut wayland::WlPointer) {}

unsafe extern "C" fn pointer_axis_source(
    _data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _axis_source: u32,
) {
}

unsafe extern "C" fn pointer_axis_stop(
    _data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _time: u32,
    _axis: u32,
) {
}

unsafe extern "C" fn pointer_axis_discrete(
    _data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _axis: u32,
    _discrete: c_int,
) {
}

unsafe extern "C" fn pointer_axis_value120(
    _data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _axis: u32,
    _value120: c_int,
) {
}

unsafe extern "C" fn pointer_axis_relative_direction(
    _data: *mut c_void,
    _pointer: *mut wayland::WlPointer,
    _axis: u32,
    _direction: u32,
) {
}

#[repr(C)]
struct XdgWmBaseListener {
    ping: Option<unsafe extern "C" fn(*mut c_void, *mut wayland::WlProxy, u32)>,
}

#[repr(C)]
struct XdgSurfaceListener {
    configure: Option<unsafe extern "C" fn(*mut c_void, *mut wayland::WlProxy, u32)>,
}

#[repr(C)]
struct XdgToplevelListener {
    configure: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut wayland::WlProxy,
            c_int,
            c_int,
            *mut wayland::WlArray,
        ),
    >,
    close: Option<unsafe extern "C" fn(*mut c_void, *mut wayland::WlProxy)>,
    configure_bounds:
        Option<unsafe extern "C" fn(*mut c_void, *mut wayland::WlProxy, c_int, c_int)>,
    wm_capabilities:
        Option<unsafe extern "C" fn(*mut c_void, *mut wayland::WlProxy, *mut wayland::WlArray)>,
}

#[repr(C)]
struct ToplevelDecorationListener {
    configure: Option<unsafe extern "C" fn(*mut c_void, *mut wayland::WlProxy, u32)>,
}

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

const POLLIN: c_short = 0x0001;

unsafe extern "C" {
    fn poll(fds: *mut PollFd, nfds: c_uint, timeout: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
}

fn run() -> Result<(), String> {
    let Some(options) = Options::parse()? else {
        print_help();
        return Ok(());
    };

    let mut window = WaylandWindow::new(options.width, options.height, WINDOW_TITLE_TEXT)?;
    let mut matrix = Matrix::new(options);
    matrix.init_gl();
    let (width, height) = window.size();
    matrix.reshape(width, height);

    while window.poll_events(&mut matrix) {
        matrix.draw_frame(&window);
        thread::sleep(DEFAULT_DELAY);
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("glmatrix-rs: {error}");
        process::exit(1);
    }
}

mod ctime {
    use super::{c_char, c_int, c_long};

    pub type TimeT = c_long;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Tm {
        pub tm_sec: c_int,
        pub tm_min: c_int,
        pub tm_hour: c_int,
        pub tm_mday: c_int,
        pub tm_mon: c_int,
        pub tm_year: c_int,
        pub tm_wday: c_int,
        pub tm_yday: c_int,
        pub tm_isdst: c_int,
        pub tm_gmtoff: c_long,
        pub tm_zone: *const c_char,
    }

    unsafe extern "C" {
        pub fn time(tloc: *mut TimeT) -> TimeT;
        pub fn localtime_r(timep: *const TimeT, result: *mut Tm) -> *mut Tm;
        pub fn strftime(s: *mut c_char, max: usize, format: *const c_char, tm: *const Tm) -> usize;
    }
}

#[allow(non_camel_case_types)]
mod gl {
    use super::{c_int, c_uint, c_void};

    pub type GLenum = c_uint;
    pub type GLbitfield = c_uint;
    pub type GLint = c_int;
    pub type GLsizei = c_int;
    pub type GLuint = c_uint;
    pub type GLfloat = f32;
    pub type GLdouble = f64;

    pub const GL_COLOR_BUFFER_BIT: GLbitfield = 0x0000_4000;
    pub const GL_DEPTH_BUFFER_BIT: GLbitfield = 0x0000_0100;
    pub const GL_LINES: GLenum = 0x0001;
    pub const GL_LINE_LOOP: GLenum = 0x0002;
    pub const GL_QUADS: GLenum = 0x0007;
    pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
    pub const GL_BLEND: GLenum = 0x0BE2;
    pub const GL_SRC_ALPHA: GLenum = 0x0302;
    pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
    pub const GL_ONE: GLenum = 1;
    pub const GL_CULL_FACE: GLenum = 0x0B44;
    pub const GL_DEPTH_TEST: GLenum = 0x0B71;
    pub const GL_NORMALIZE: GLenum = 0x0BA1;
    pub const GL_SMOOTH: GLenum = 0x1D01;
    pub const GL_PROJECTION: GLenum = 0x1701;
    pub const GL_MODELVIEW: GLenum = 0x1700;
    pub const GL_RGBA: GLenum = 0x1908;
    pub const GL_UNSIGNED_BYTE: GLenum = 0x1401;
    pub const GL_UNPACK_ALIGNMENT: GLenum = 0x0CF5;
    pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
    pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
    pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
    pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
    pub const GL_LINEAR: GLenum = 0x2601;
    pub const GL_REPEAT: GLenum = 0x2901;
    pub const GL_TEXTURE_ENV: GLenum = 0x2300;
    pub const GL_TEXTURE_ENV_MODE: GLenum = 0x2200;
    pub const GL_MODULATE: GLenum = 0x2100;

    unsafe extern "C" {
        pub fn glBegin(mode: GLenum);
        pub fn glBindTexture(target: GLenum, texture: GLuint);
        pub fn glBlendFunc(sfactor: GLenum, dfactor: GLenum);
        pub fn glClear(mask: GLbitfield);
        pub fn glClearColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat);
        pub fn glColor4f(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat);
        pub fn glDeleteTextures(n: GLsizei, textures: *const GLuint);
        pub fn glDisable(cap: GLenum);
        pub fn glEnable(cap: GLenum);
        pub fn glEnd();
        pub fn glFinish();
        pub fn glFrustum(
            left: GLdouble,
            right: GLdouble,
            bottom: GLdouble,
            top: GLdouble,
            near_val: GLdouble,
            far_val: GLdouble,
        );
        pub fn glGenTextures(n: GLsizei, textures: *mut GLuint);
        pub fn glLoadIdentity();
        pub fn glMatrixMode(mode: GLenum);
        pub fn glNormal3f(nx: GLfloat, ny: GLfloat, nz: GLfloat);
        pub fn glOrtho(
            left: GLdouble,
            right: GLdouble,
            bottom: GLdouble,
            top: GLdouble,
            near_val: GLdouble,
            far_val: GLdouble,
        );
        pub fn glPixelStorei(pname: GLenum, param: GLint);
        pub fn glPopMatrix();
        pub fn glPushMatrix();
        pub fn glRotatef(angle: GLfloat, x: GLfloat, y: GLfloat, z: GLfloat);
        pub fn glShadeModel(mode: GLenum);
        pub fn glTexCoord2f(s: GLfloat, t: GLfloat);
        pub fn glTexEnvi(target: GLenum, pname: GLenum, param: GLint);
        pub fn glTexImage2D(
            target: GLenum,
            level: GLint,
            internalformat: GLint,
            width: GLsizei,
            height: GLsizei,
            border: GLint,
            format: GLenum,
            type_: GLenum,
            pixels: *const c_void,
        );
        pub fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);
        pub fn glTranslatef(x: GLfloat, y: GLfloat, z: GLfloat);
        pub fn glVertex3f(x: GLfloat, y: GLfloat, z: GLfloat);
        pub fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
    }
}

#[allow(non_camel_case_types)]
mod egl {
    use super::{c_int, c_uint, c_void};

    pub type EGLBoolean = c_uint;
    pub type EGLenum = c_uint;
    pub type EGLint = c_int;
    pub type EGLDisplay = *mut c_void;
    pub type EGLConfig = *mut c_void;
    pub type EGLSurface = *mut c_void;
    pub type EGLContext = *mut c_void;

    pub const EGL_FALSE: EGLBoolean = 0;
    pub const EGL_NONE: EGLint = 0x3038;
    pub const EGL_WINDOW_BIT: EGLint = 0x0004;
    pub const EGL_OPENGL_BIT: EGLint = 0x0008;
    pub const EGL_SURFACE_TYPE: EGLint = 0x3033;
    pub const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
    pub const EGL_RED_SIZE: EGLint = 0x3024;
    pub const EGL_GREEN_SIZE: EGLint = 0x3023;
    pub const EGL_BLUE_SIZE: EGLint = 0x3022;
    pub const EGL_ALPHA_SIZE: EGLint = 0x3021;
    pub const EGL_DEPTH_SIZE: EGLint = 0x3025;
    pub const EGL_OPENGL_API: EGLenum = 0x30A2;
    pub const EGL_CONTEXT_MAJOR_VERSION: EGLint = 0x3098;
    pub const EGL_CONTEXT_MINOR_VERSION: EGLint = 0x30FB;

    unsafe extern "C" {
        pub fn eglGetDisplay(display_id: *mut c_void) -> EGLDisplay;
        pub fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint)
            -> EGLBoolean;
        pub fn eglTerminate(dpy: EGLDisplay) -> EGLBoolean;
        pub fn eglBindAPI(api: EGLenum) -> EGLBoolean;
        pub fn eglChooseConfig(
            dpy: EGLDisplay,
            attrib_list: *const EGLint,
            configs: *mut EGLConfig,
            config_size: EGLint,
            num_config: *mut EGLint,
        ) -> EGLBoolean;
        pub fn eglCreateContext(
            dpy: EGLDisplay,
            config: EGLConfig,
            share_context: EGLContext,
            attrib_list: *const EGLint,
        ) -> EGLContext;
        pub fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean;
        pub fn eglCreateWindowSurface(
            dpy: EGLDisplay,
            config: EGLConfig,
            win: *mut c_void,
            attrib_list: *const EGLint,
        ) -> EGLSurface;
        pub fn eglDestroySurface(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
        pub fn eglMakeCurrent(
            dpy: EGLDisplay,
            draw: EGLSurface,
            read: EGLSurface,
            ctx: EGLContext,
        ) -> EGLBoolean;
        pub fn eglSwapBuffers(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
        pub fn eglSwapInterval(dpy: EGLDisplay, interval: EGLint) -> EGLBoolean;
        pub fn eglGetError() -> EGLint;
    }
}

#[allow(non_camel_case_types)]
mod wayland_egl {
    use super::c_int;
    use super::wayland;

    #[repr(C)]
    pub struct WlEglWindow {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn wl_egl_window_create(
            surface: *mut wayland::WlSurface,
            width: c_int,
            height: c_int,
        ) -> *mut WlEglWindow;
        pub fn wl_egl_window_destroy(egl_window: *mut WlEglWindow);
        pub fn wl_egl_window_resize(
            egl_window: *mut WlEglWindow,
            width: c_int,
            height: c_int,
            dx: c_int,
            dy: c_int,
        );
    }
}

#[allow(non_camel_case_types, non_upper_case_globals)]
mod wayland {
    use super::{c_char, c_int, c_void};

    #[repr(C)]
    pub struct WlDisplay {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct WlProxy {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct WlRegistry {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct WlCompositor {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct WlSurface {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct WlSeat {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct WlKeyboard {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct WlPointer {
        _private: [u8; 0],
    }

    pub type WlFixed = i32;

    #[repr(C)]
    pub struct WlArray {
        pub size: usize,
        pub alloc: usize,
        pub data: *mut c_void,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct WlMessage {
        pub name: *const c_char,
        pub signature: *const c_char,
        pub types: *const *const WlInterface,
    }

    #[repr(C)]
    pub struct WlInterface {
        pub name: *const c_char,
        pub version: c_int,
        pub method_count: c_int,
        pub methods: *const WlMessage,
        pub event_count: c_int,
        pub events: *const WlMessage,
    }

    #[repr(C)]
    pub struct WlRegistryListener {
        pub global:
            Option<unsafe extern "C" fn(*mut c_void, *mut WlRegistry, u32, *const c_char, u32)>,
        pub global_remove: Option<unsafe extern "C" fn(*mut c_void, *mut WlRegistry, u32)>,
    }

    #[repr(C)]
    pub struct WlSeatListener {
        pub capabilities: Option<unsafe extern "C" fn(*mut c_void, *mut WlSeat, u32)>,
        pub name: Option<unsafe extern "C" fn(*mut c_void, *mut WlSeat, *const c_char)>,
    }

    #[repr(C)]
    pub struct WlKeyboardListener {
        pub keymap: Option<unsafe extern "C" fn(*mut c_void, *mut WlKeyboard, u32, c_int, u32)>,
        pub enter: Option<
            unsafe extern "C" fn(*mut c_void, *mut WlKeyboard, u32, *mut WlSurface, *mut WlArray),
        >,
        pub leave: Option<unsafe extern "C" fn(*mut c_void, *mut WlKeyboard, u32, *mut WlSurface)>,
        pub key: Option<unsafe extern "C" fn(*mut c_void, *mut WlKeyboard, u32, u32, u32, u32)>,
        pub modifiers:
            Option<unsafe extern "C" fn(*mut c_void, *mut WlKeyboard, u32, u32, u32, u32, u32)>,
        pub repeat_info: Option<unsafe extern "C" fn(*mut c_void, *mut WlKeyboard, c_int, c_int)>,
    }

    #[repr(C)]
    pub struct WlPointerListener {
        pub enter: Option<
            unsafe extern "C" fn(
                *mut c_void,
                *mut WlPointer,
                u32,
                *mut WlSurface,
                WlFixed,
                WlFixed,
            ),
        >,
        pub leave: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, *mut WlSurface)>,
        pub motion:
            Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, WlFixed, WlFixed)>,
        pub button: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, u32, u32, u32)>,
        pub axis: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, u32, WlFixed)>,
        pub frame: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer)>,
        pub axis_source: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32)>,
        pub axis_stop: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, u32)>,
        pub axis_discrete: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, c_int)>,
        pub axis_value120: Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, c_int)>,
        pub axis_relative_direction:
            Option<unsafe extern "C" fn(*mut c_void, *mut WlPointer, u32, u32)>,
    }

    pub const WL_DISPLAY_GET_REGISTRY: u32 = 1;
    pub const WL_REGISTRY_BIND: u32 = 0;
    pub const WL_COMPOSITOR_CREATE_SURFACE: u32 = 0;
    pub const WL_SURFACE_COMMIT: u32 = 6;
    pub const WL_SEAT_GET_POINTER: u32 = 0;
    pub const WL_SEAT_GET_KEYBOARD: u32 = 1;

    unsafe extern "C" {
        pub static wl_registry_interface: WlInterface;
        pub static wl_compositor_interface: WlInterface;
        pub static wl_surface_interface: WlInterface;
        pub static wl_seat_interface: WlInterface;
        pub static wl_keyboard_interface: WlInterface;
        pub static wl_pointer_interface: WlInterface;

        pub fn wl_display_connect(name: *const c_char) -> *mut WlDisplay;
        pub fn wl_display_disconnect(display: *mut WlDisplay);
        pub fn wl_display_dispatch(display: *mut WlDisplay) -> c_int;
        pub fn wl_display_dispatch_pending(display: *mut WlDisplay) -> c_int;
        pub fn wl_display_get_error(display: *mut WlDisplay) -> c_int;
        pub fn wl_display_flush(display: *mut WlDisplay) -> c_int;
        pub fn wl_display_get_fd(display: *mut WlDisplay) -> c_int;
        pub fn wl_display_prepare_read(display: *mut WlDisplay) -> c_int;
        pub fn wl_display_cancel_read(display: *mut WlDisplay);
        pub fn wl_display_read_events(display: *mut WlDisplay) -> c_int;
        pub fn wl_display_roundtrip(display: *mut WlDisplay) -> c_int;

        pub fn wl_proxy_marshal_flags(
            proxy: *mut WlProxy,
            opcode: u32,
            interface: *const WlInterface,
            version: u32,
            flags: u32,
            ...
        ) -> *mut WlProxy;
        pub fn wl_proxy_destroy(proxy: *mut WlProxy);
        pub fn wl_proxy_add_listener(
            proxy: *mut WlProxy,
            implementation: *const c_void,
            data: *mut c_void,
        ) -> c_int;
        pub fn wl_proxy_get_version(proxy: *mut WlProxy) -> u32;
    }
}
