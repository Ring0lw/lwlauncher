use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont, point};
use glow::HasContext;
use std::collections::HashMap;
use std::io::Read;

const BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSans-Bold.ttf");
const MEDIUM: &[u8] = include_bytes!("../assets/fonts/NotoSans-Medium.ttf");
const ICONS: &[u8] = include_bytes!("../assets/fonts/Icons.ttf");
const LOGO_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/logo.z"));
pub const LOGO_SIZE: usize = 256;
const ATLAS: usize = 1024;

// COLOR
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub f32, pub f32, pub f32, pub f32);

impl Rgba {
    pub const NONE: Rgba = Rgba(0.0, 0.0, 0.0, 0.0);
    pub const fn hex(v: u32) -> Rgba {
        Rgba(((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0, 1.0)
    }
    pub fn a(self, a: f32) -> Rgba {
        Rgba(self.0, self.1, self.2, self.3 * a.clamp(0.0, 1.0))
    }
    pub fn mix(self, o: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        Rgba(self.0 + (o.0 - self.0) * t, self.1 + (o.1 - self.1) * t, self.2 + (o.2 - self.2) * t, self.3 + (o.3 - self.3) * t)
    }
    fn pm(self) -> [f32; 4] {
        [self.0 * self.3, self.1 * self.3, self.2 * self.3, self.3]
    }
}

// INSTANCE
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Inst {
    rect: [f32; 4],
    uv: [f32; 4],
    fill: [f32; 4],
    border: [f32; 4],
    glow: [f32; 4],
    extra: [f32; 4],
    p0: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
}

impl Default for Inst {
    fn default() -> Inst {
        Inst { rect: [0.0; 4], uv: [0.0; 4], fill: [0.0; 4], border: [0.0; 4], glow: [0.0; 4], extra: [0.0; 4], p0: [0.0; 4], p1: [-2.0, 0.0, 0.0, 0.0], p2: [0.0; 4] }
    }
}

#[derive(Clone, Copy, Default)]
pub struct BoxStyle {
    pub fill: Option<Rgba>,
    pub border: Option<(Rgba, f32)>,
    pub radius: f32,
    pub glow: Option<(Rgba, f32)>,
    pub shimmer: Option<f32>,
    pub progress: Option<(f32, Rgba, f32)>,
    pub ripple: Option<(f32, f32, f32, f32)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Font {
    Bold,
    Medium,
    Icon,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
}

#[derive(Clone, Copy)]
struct Glyph {
    uv: [f32; 4],
    off: [f32; 2],
    size: [f32; 2],
}

pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

// TEXT
pub struct Text {
    fonts: [FontRef<'static>; 3],
    faces: [ttf_parser::Face<'static>; 2],
    kern: HashMap<(u8, u16, u16), f32>,
    cache: HashMap<(Font, char, u32, u8), Option<Glyph>>,
    pixels: Vec<u8>,
    shelf: (usize, usize, usize),
    dirty: bool,
    pub scale: f32,
}

impl Text {
    fn new(scale: f32) -> Text {
        Text {
            fonts: [FontRef::try_from_slice(BOLD).unwrap(), FontRef::try_from_slice(MEDIUM).unwrap(), FontRef::try_from_slice(ICONS).unwrap()],
            faces: [ttf_parser::Face::parse(BOLD, 0).unwrap(), ttf_parser::Face::parse(MEDIUM, 0).unwrap()],
            kern: HashMap::new(),
            cache: HashMap::new(),
            pixels: vec![0; ATLAS * ATLAS],
            shelf: (1, 1, 0),
            dirty: true,
            scale,
        }
    }

    fn font(&self, f: Font) -> &FontRef<'static> {
        &self.fonts[f as usize]
    }

    fn kerning(&mut self, f: Font, a: ab_glyph::GlyphId, b: ab_glyph::GlyphId) -> f32 {
        if f == Font::Icon {
            return 0.0;
        }
        let key = (f as u8, a.0, b.0);
        if let Some(v) = self.kern.get(&key) {
            return *v;
        }
        let v = gpos_kern(&self.faces[f as usize], ttf_parser::GlyphId(a.0), ttf_parser::GlyphId(b.0)) as f32 / 1000.0;
        self.kern.insert(key, v);
        v
    }

    pub fn measure(&mut self, f: Font, size: f32, s: &str) -> f32 {
        let px = size * self.scale;
        let font = self.fonts[f as usize].clone();
        let sf = font.as_scaled(PxScale::from(px));
        let mut w = 0.0;
        let mut prev = None;
        for c in s.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                w += self.kerning(f, p, id) * px;
            }
            w += sf.h_advance(id);
            prev = Some(id);
        }
        w / self.scale
    }

    pub fn ellipsize(&mut self, f: Font, size: f32, s: &str, max: f32) -> String {
        if self.measure(f, size, s) <= max {
            return s.to_string();
        }
        let mut chars: Vec<char> = s.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let t: String = chars.iter().collect::<String>() + "\u{2026}";
            if self.measure(f, size, &t) <= max {
                return t;
            }
        }
        String::new()
    }

    pub fn wrap(&mut self, f: Font, size: f32, s: &str, max: f32) -> Vec<String> {
        let mut lines = Vec::new();
        for para in s.split('\n') {
            let mut cur = String::new();
            for word in para.split_whitespace() {
                let next = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
                if !cur.is_empty() && self.measure(f, size, &next) > max {
                    lines.push(std::mem::take(&mut cur));
                    cur = word.to_string();
                } else {
                    cur = next;
                }
            }
            lines.push(cur);
        }
        lines
    }

    fn glyph(&mut self, f: Font, c: char, px: f32, sub: u8) -> Option<Glyph> {
        let key = (f, c, (px * 16.0) as u32, sub);
        if let Some(g) = self.cache.get(&key) {
            return *g;
        }
        let font = self.fonts[f as usize].clone();
        let id = font.glyph_id(c);
        let g = font.outline_glyph(id.with_scale_and_position(PxScale::from(px), point(sub as f32 / 4.0, 0.0))).and_then(|og| {
            let b = og.px_bounds();
            let (w, h) = (b.width() as usize, b.height() as usize);
            if w == 0 || h == 0 {
                return None;
            }
            let (mut x, mut y, mut sh) = self.shelf;
            if x + w + 1 >= ATLAS {
                x = 1;
                y += sh + 1;
                sh = 0;
            }
            if y + h + 1 >= ATLAS {
                self.pixels.fill(0);
                self.cache.clear();
                x = 1;
                y = 1;
                sh = 0;
            }
            let pixels = &mut self.pixels;
            og.draw(|gx, gy, v| {
                let i = (y + gy as usize) * ATLAS + x + gx as usize;
                pixels[i] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            });
            self.shelf = (x + w + 1, y, sh.max(h));
            self.dirty = true;
            let a = ATLAS as f32;
            Some(Glyph {
                uv: [x as f32 / a, y as f32 / a, (x + w) as f32 / a, (y + h) as f32 / a],
                off: [b.min.x, b.min.y],
                size: [w as f32, h as f32],
            })
        });
        self.cache.insert(key, g);
        g
    }

    pub fn mask(&mut self, f: Font, size: f32, s: &str, pad: usize) -> Mask {
        let px = size * self.scale;
        let font = self.fonts[f as usize].clone();
        let sf = font.as_scaled(PxScale::from(px));
        let w = (self.measure(f, size, s) * self.scale).ceil() as usize + pad * 2;
        let h = (sf.ascent() - sf.descent()).ceil() as usize + pad * 2;
        let mut data = vec![0u8; w * h];
        let mut pen = pad as f32;
        let base = pad as f32 + sf.ascent();
        let mut prev = None;
        for c in s.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                pen += self.kerning(f, p, id) * px;
            }
            if let Some(og) = font.outline_glyph(id.with_scale_and_position(PxScale::from(px), point(pen, base))) {
                let b = og.px_bounds();
                og.draw(|gx, gy, v| {
                    let (x, y) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
                    if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                        let i = y as usize * w + x as usize;
                        data[i] = data[i].max((v.clamp(0.0, 1.0) * 255.0) as u8);
                    }
                });
            }
            pen += sf.h_advance(id);
            prev = Some(id);
        }
        Mask { w, h, data }
    }

    pub fn ascent(&self, f: Font, size: f32) -> f32 {
        self.font(f).as_scaled(PxScale::from(size)).ascent()
    }
}

fn gpos_kern(face: &ttf_parser::Face, a: ttf_parser::GlyphId, b: ttf_parser::GlyphId) -> i16 {
    use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
    let Some(gpos) = face.tables().gpos else { return 0 };
    let mut total = 0;
    for lookup in gpos.lookups {
        for sub in lookup.subtables.into_iter::<PositioningSubtable>() {
            let PositioningSubtable::Pair(pa) = sub else { continue };
            let v = match pa {
                PairAdjustment::Format1 { coverage, sets } => coverage.get(a).and_then(|i| sets.get(i)).and_then(|s| s.get(b)).map(|(v, _)| v.x_advance),
                PairAdjustment::Format2 { coverage, classes, matrix } => {
                    coverage.get(a).and_then(|_| matrix.get((classes.0.get(a), classes.1.get(b)))).map(|(v, _)| v.x_advance)
                }
            };
            if let Some(v) = v {
                total += v;
                break;
            }
        }
    }
    total
}

pub fn blur(m: &mut Mask, radius: usize) {
    if radius == 0 {
        return;
    }
    let (w, h) = (m.w, m.h);
    let mut buf: Vec<f32> = m.data.iter().map(|v| *v as f32).collect();
    let mut tmp = vec![0f32; w * h];
    let norm = 1.0 / (2 * radius + 1) as f32;
    for _ in 0..3 {
        for y in 0..h {
            let row = y * w;
            let mut acc = 0.0;
            for i in 0..=radius.min(w - 1) {
                acc += buf[row + i];
            }
            for x in 0..w {
                tmp[row + x] = acc * norm;
                if x + radius + 1 < w {
                    acc += buf[row + x + radius + 1];
                }
                if x >= radius {
                    acc -= buf[row + x - radius];
                }
            }
        }
        for x in 0..w {
            let mut acc = 0.0;
            for i in 0..=radius.min(h - 1) {
                acc += tmp[i * w + x];
            }
            for y in 0..h {
                buf[y * w + x] = acc * norm;
                if y + radius + 1 < h {
                    acc += tmp[(y + radius + 1) * w + x];
                }
                if y >= radius {
                    acc -= tmp[(y - radius) * w + x];
                }
            }
        }
    }
    for (d, v) in m.data.iter_mut().zip(buf) {
        *d = v.clamp(0.0, 255.0) as u8;
    }
}

pub fn logo_rgba() -> Vec<u8> {
    let mut out = Vec::with_capacity(LOGO_SIZE * LOGO_SIZE * 4);
    flate2::read::ZlibDecoder::new(LOGO_Z).read_to_end(&mut out).expect("logo");
    out
}

// GFX
pub struct Gfx {
    pub gl: glow::Context,
    bg: glow::Program,
    ui: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    atlas: glow::Texture,
    logo: glow::Texture,
    fx: glow::Texture,
    pub fx_size: (usize, usize),
    pub text: Text,
    pub insts: Vec<Inst>,
    pub oy: f32,
    pub res: (f32, f32),
}

unsafe fn compile(gl: &glow::Context, header: &str, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let p = gl.create_program()?;
        for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let s = gl.create_shader(kind)?;
            gl.shader_source(s, &format!("{header}{src}"));
            gl.compile_shader(s);
            if !gl.get_shader_compile_status(s) {
                return Err(gl.get_shader_info_log(s));
            }
            gl.attach_shader(p, s);
        }
        gl.link_program(p);
        if !gl.get_program_link_status(p) {
            return Err(gl.get_program_info_log(p));
        }
        Ok(p)
    }
}

const FULLSCREEN_VS: &str = "void main() { vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0); }\n";

unsafe fn texture(gl: &glow::Context, filter: u32) -> glow::Texture {
    unsafe {
        let t = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(t));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        t
    }
}

impl Gfx {
    pub fn new(gl: glow::Context, gles: bool, scale: f32) -> Result<Gfx, String> {
        let header = if gles { "#version 300 es\nprecision highp float;\nprecision highp int;\n" } else { "#version 330 core\n" };
        unsafe {
            let bg = compile(&gl, header, FULLSCREEN_VS, include_str!("shaders/bg.frag"))?;
            let ui = compile(&gl, header, include_str!("shaders/ui.vert"), include_str!("shaders/ui.frag"))?;
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let stride = std::mem::size_of::<Inst>() as i32;
            for i in 0..9u32 {
                gl.enable_vertex_attrib_array(i);
                gl.vertex_attrib_pointer_f32(i, 4, glow::FLOAT, false, stride, (i * 16) as i32);
                gl.vertex_attrib_divisor(i, 1);
            }
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            let atlas = texture(&gl, glow::LINEAR);
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R8 as i32, ATLAS as i32, ATLAS as i32, 0, glow::RED, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None));
            let logo = texture(&gl, glow::LINEAR_MIPMAP_LINEAR);
            let px = logo_rgba();
            let n = LOGO_SIZE as i32;
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, n, n, 0, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(&px)));
            gl.generate_mipmap(glow::TEXTURE_2D);
            let fx = texture(&gl, glow::LINEAR);
            gl.use_program(Some(ui));
            for (name, unit) in [("u_atlas", 0), ("u_image", 1), ("u_fx", 2)] {
                gl.uniform_1_i32(gl.get_uniform_location(ui, name).as_ref(), unit);
            }
            Ok(Gfx { gl, bg, ui, vao, vbo, atlas, logo, fx, fx_size: (1, 1), text: Text::new(scale), insts: Vec::with_capacity(256), res: (1.0, 1.0), oy: 0.0 })
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        if (self.text.scale - scale).abs() > 1e-3 {
            self.text = Text::new(scale);
        }
    }

    pub fn upload_fx(&mut self, m: &Mask) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.fx));
            self.gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R8 as i32, m.w as i32, m.h as i32, 0, glow::RED, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(&m.data)));
        }
        self.fx_size = (m.w, m.h);
    }

    fn s(&self) -> f32 {
        self.text.scale
    }

    // DRAW
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, st: BoxStyle) {
        let s = self.s();
        let y = y + self.oy;
        let mut i = Inst { rect: [x * s, y * s, w * s, h * s], ..Default::default() };
        i.fill = st.fill.unwrap_or(Rgba::NONE).pm();
        if let Some((c, bw)) = st.border {
            i.border = c.pm();
            i.p0[1] = bw * s;
        }
        i.p0[0] = (st.radius * s).min(w.min(h) * s * 0.5);
        if let Some((c, r)) = st.glow {
            if c.3 > 0.003 {
                i.glow = c.pm();
                i.p0[2] = r * s;
            }
        }
        if let Some(v) = st.shimmer {
            i.p1[0] = v;
        }
        if let Some((p, c, phase)) = st.progress {
            i.p1[1] = p.max(0.0001);
            i.extra = c.pm();
            i.p1[2] = phase;
        }
        if let Some((rx, ry, rr, ra)) = st.ripple {
            i.p2 = [rx * s, ry * s, rr * s, ra];
        }
        self.insts.push(i);
    }

    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, c: Rgba) {
        self.rect(x, y, w, h, BoxStyle { fill: Some(c), radius, ..Default::default() });
    }

    pub fn spinner(&mut self, cx: f32, cy: f32, r: f32, thick: f32, c: Rgba, rot: f32) {
        let s = self.s();
        let cy = cy + self.oy;
        let mut i = Inst { rect: [(cx - r) * s, (cy - r) * s, 2.0 * r * s, 2.0 * r * s], ..Default::default() };
        i.fill = c.pm();
        i.p0 = [0.0, thick * s, 0.0, 4.0];
        i.p1 = [rot, 4.4, 0.0, 0.0];
        self.insts.push(i);
    }

    pub fn image(&mut self, x: f32, y: f32, w: f32, h: f32, alpha: f32) {
        let s = self.s();
        let y = y + self.oy;
        let mut i = Inst { rect: [x * s, y * s, w * s, h * s], uv: [0.0, 0.0, 1.0, 1.0], ..Default::default() };
        i.fill = [alpha, alpha, alpha, alpha];
        i.p0[3] = 2.0;
        self.insts.push(i);
    }

    pub fn fx(&mut self, x: f32, y: f32, uv: [f32; 4], c: Rgba) {
        let s = self.s();
        let y = y + self.oy;
        let (fw, fh) = (self.fx_size.0 as f32, self.fx_size.1 as f32);
        let (w, h) = ((uv[2] - uv[0]) * fw, (uv[3] - uv[1]) * fh);
        let mut i = Inst { rect: [(x * s).round(), (y * s).round(), w, h], uv, ..Default::default() };
        i.fill = c.pm();
        i.p0[3] = 3.0;
        self.insts.push(i);
    }

    pub fn text(&mut self, f: Font, size: f32, s: &str, x: f32, base: f32, align: Align, c: Rgba) -> f32 {
        if c.3 <= 0.002 {
            return 0.0;
        }
        let sc = self.s();
        let w = self.text.measure(f, size, s);
        let x0 = match align {
            Align::Left => x,
            Align::Center => x - w * 0.5,
        };
        let px = size * sc;
        let font = self.text.font(f).clone();
        let sf = font.as_scaled(PxScale::from(px));
        let mut pen = x0 * sc;
        let by = ((base + self.oy) * sc).round();
        let mut prev = None;
        let fill = c.pm();
        for ch in s.chars() {
            let id = font.glyph_id(ch);
            if let Some(p) = prev {
                pen += self.text.kerning(f, p, id) * px;
            }
            let sub = ((pen.fract() * 4.0) as u8) & 3;
            if let Some(g) = self.text.glyph(f, ch, px, sub) {
                let mut i = Inst { rect: [pen.floor() + g.off[0], by + g.off[1], g.size[0], g.size[1]], uv: g.uv, ..Default::default() };
                i.fill = fill;
                i.p0[3] = 1.0;
                self.insts.push(i);
            }
            pen += sf.h_advance(id);
            prev = Some(id);
        }
        w
    }

    pub fn render(&mut self, w: u32, h: u32, time: f32, mouse: (f32, f32), motion: f32, logo: [f32; 4]) {
        let gl = &self.gl;
        let s = self.text.scale;
        unsafe {
            gl.viewport(0, 0, w as i32, h as i32);
            gl.disable(glow::BLEND);
            gl.bind_vertex_array(Some(self.vao));
            gl.use_program(Some(self.bg));
            let u = |n: &str| gl.get_uniform_location(self.bg, n);
            gl.uniform_2_f32(u("u_res").as_ref(), w as f32, h as f32);
            gl.uniform_1_f32(u("u_scale").as_ref(), s);
            gl.uniform_1_f32(u("u_time").as_ref(), time);
            gl.uniform_2_f32(u("u_mouse").as_ref(), mouse.0, mouse.1);
            gl.uniform_1_f32(u("u_motion").as_ref(), motion);
            gl.uniform_4_f32(u("u_logo").as_ref(), logo[0], logo[1], logo[2], logo[3]);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            if self.text.dirty {
                gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas));
                gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, ATLAS as i32, ATLAS as i32, glow::RED, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(&self.text.pixels)));
                self.text.dirty = false;
            }
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.ui));
            let u = |n: &str| gl.get_uniform_location(self.ui, n);
            gl.uniform_2_f32(u("u_res").as_ref(), w as f32, h as f32);
            gl.uniform_1_f32(u("u_time").as_ref(), time);
            gl.uniform_1_f32(u("u_scale").as_ref(), s);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.logo));
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.fx));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            let bytes = std::slice::from_raw_parts(self.insts.as_ptr() as *const u8, self.insts.len() * std::mem::size_of::<Inst>());
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
            gl.draw_arrays_instanced(glow::TRIANGLE_STRIP, 0, 4, self.insts.len() as i32);
        }
        self.insts.clear();
    }
}
