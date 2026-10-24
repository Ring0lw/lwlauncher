use crate::askpass::{self, Bridge, Request};
use crate::gfx::{Align, BoxStyle, Font, Gfx, Rgba, blur};
use crate::install::{self, Outcome, Progress};
use crate::launch;
use crate::net::{self, Status};
use crate::remove;
use crate::{Config, Paths};
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext, PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{CursorIcon, Icon, Window, WindowId};

mod draw;
use draw::window_icon;

// PALETTE
const BG: Rgba = Rgba::hex(0x050f08);
const CIRCLE: Rgba = Rgba::hex(0x141e17);
const CIRCLE_HI: Rgba = Rgba::hex(0x1c2c21);
const MINT: Rgba = Rgba::hex(0x8cc8a0);
const TITLE: Rgba = Rgba::hex(0xa8f0c8);
const GREEN: Rgba = Rgba::hex(0x00cc66);
const GREEN_HI: Rgba = Rgba::hex(0x19dd77);
const GLOW: Rgba = Rgba::hex(0x22d36b);
const DARK_TEXT: Rgba = Rgba::hex(0x0a1a0e);
const BTN: Rgba = Rgba::hex(0x1a1a1a);
const BTN_BORDER: Rgba = Rgba::hex(0x333333);
const BTN_TEXT_OFF: Rgba = Rgba::hex(0x4a4a4a);
const BTN_ON: Rgba = Rgba::hex(0x16221a);
const BTN_ON_BORDER: Rgba = Rgba::hex(0x2e6e48);
const BTN_ON_TEXT: Rgba = Rgba::hex(0xcdeedb);
const RED: Rgba = Rgba::hex(0xe5484d);
const DEL: Rgba = Rgba::hex(0x1d1617);
const DEL_BORDER: Rgba = Rgba::hex(0x4a2b2d);
const DEL_TEXT: Rgba = Rgba::hex(0xe8b4b6);
const MUTED: Rgba = Rgba::hex(0x5d7d68);
const BODY: Rgba = Rgba::hex(0xb9d9c5);
const SUBTLE: Rgba = Rgba::hex(0x6f8f7a);
const OK: Rgba = Rgba::hex(0x5fd38d);
const WARN: Rgba = Rgba::hex(0xe5c07b);
const ERR: Rgba = Rgba::hex(0xff7b72);
const WHITE: Rgba = Rgba::hex(0xeef5f0);

const ICON_GEAR: &str = "\u{f013}";
const ICON_HELP: &str = "\u{f128}";
const ICON_X: &str = "\u{f00d}";
const ICON_CHECK: &str = "\u{f058}";
const ICON_LOCK: &str = "\u{f023}";

const W: f32 = 500.0;
const TOP: f32 = 30.0;
const H: f32 = 352.0;
const ASK_W: f32 = 380.0;
const ASK_H: f32 = 184.0;
const HOLD: f32 = 1.2;
const DISCORD: &str = "https://discord.gg/2FBZyX7ysb";
const DISCORD_BACKUP: &str = "https://fluxer.gg/Gz4KEbfu";

pub enum Mode {
    Launcher,
    Askpass(String),
}

pub enum Msg {
    Checked(Result<Status, String>),
    Updated(Result<(Outcome, Option<String>), String>),
    Ask(Request),
    Launch(launch::Event),
    Deleted(Result<remove::Report, String>),
    RootRm(bool, String),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Id {
    Gear,
    Help,
    External,
    Delete,
    Update,
    TogAuto,
    TogClose,
    OpenFolder,
    Discord,
    Backup,
    OpenLog,
    Changelog,
    Field,
    PwCancel,
    PwOk,
}

#[derive(Clone, Copy, PartialEq)]
enum Panel {
    Main,
    Settings,
    Help,
}

#[derive(PartialEq)]
enum Job {
    Idle,
    Checking,
    Updating,
    Launching,
    Deleting,
}

#[derive(Default)]
struct Anim {
    hover: f32,
    press: f32,
    on: f32,
    scale: f32,
    vel: f32,
    ripple: Option<(f32, f32, Instant)>,
}

struct Hit {
    id: Id,
    r: [f32; 4],
    enabled: bool,
}

struct Tone(String, Rgba);

struct Gl {
    window: Arc<Window>,
    surface: Surface<WindowSurface>,
    ctx: PossiblyCurrentContext,
    gfx: Gfx,
    title_glow: [f32; 4],
    logo_glow: [f32; 4],
    glow_pad: (f32, f32),
}

struct App {
    paths: Paths,
    mode: Mode,
    cfg: Config,
    proxy: EventLoopProxy<Msg>,
    gl: Option<Gl>,
    err: Option<String>,
    code: i32,
    t0: Instant,
    last: Instant,
    next: Instant,
    focused: bool,
    hidden: bool,
    mouse: (f32, f32),
    mouse_in: bool,
    mouse_s: (f32, f32),
    mouse_k: f32,
    hits: Vec<Hit>,
    anims: HashMap<Id, Anim>,
    knobs: HashMap<Id, f32>,
    pressed: Option<Id>,
    panel: Panel,
    panel_t: f32,
    prev_panel: Panel,
    job: Job,
    progress: Arc<Progress>,
    remote: Option<Result<Status, String>>,
    tone: Option<(Tone, Instant)>,
    changelog: bool,
    done_flash: Option<Instant>,
    bridge: Option<Bridge>,
    ask: Option<Request>,
    ask_tries: u32,
    ask_t: f32,
    shake: Option<Instant>,
    password: String,
    caret_t: Instant,
    launched: bool,
    fade: Option<Instant>,
    root_rm: Option<remove::Plan>,
    hold: f32,
    intro: Instant,
    done: bool,
    shown: bool,
    trimmed: bool,
    dt: f32,
}

unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

fn ease(cur: &mut f32, target: f32, dt: f32, speed: f32) {
    *cur += (target - *cur) * (1.0 - (-dt * speed).exp());
}

fn inside(r: [f32; 4], p: (f32, f32)) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

pub fn run(paths: Paths, mode: Mode) -> Result<i32, String> {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err("no DISPLAY or WAYLAND_DISPLAY".into());
    }
    let el = EventLoop::<Msg>::with_user_event().build().map_err(|e| e.to_string())?;
    let proxy = el.create_proxy();
    let cfg = Config::load(&paths);
    let now = Instant::now();
    let mut app = App {
        paths,
        mode,
        cfg,
        proxy,
        gl: None,
        err: None,
        code: 0,
        t0: now,
        last: now,
        next: now,
        focused: true,
        hidden: false,
        mouse: (-1e4, -1e4),
        mouse_in: false,
        mouse_s: (-1e4, -1e4),
        mouse_k: 0.0,
        hits: Vec::new(),
        anims: HashMap::new(),
        knobs: HashMap::new(),
        pressed: None,
        panel: Panel::Main,
        panel_t: 1.0,
        prev_panel: Panel::Main,
        job: Job::Idle,
        progress: Arc::new(Progress::default()),
        remote: None,
        tone: None,
        changelog: false,
        done_flash: None,
        bridge: None,
        ask: None,
        ask_tries: 0,
        ask_t: 0.0,
        shake: None,
        password: String::new(),
        caret_t: now,
        launched: false,
        fade: None,
        root_rm: None,
        hold: 0.0,
        intro: now,
        done: false,
        shown: false,
        trimmed: false,
        dt: 0.016,
    };
    el.run_app(&mut app).map_err(|e| e.to_string())?;
    if let Some(e) = app.err {
        return Err(e);
    }
    Ok(app.code)
}

impl App {
    fn size(&self) -> (f32, f32) {
        match self.mode {
            Mode::Launcher => (W, H),
            Mode::Askpass(_) => (ASK_W, ASK_H),
        }
    }

    fn create(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        use winit::platform::wayland::WindowAttributesExtWayland;
        use winit::platform::x11::{WindowAttributesExtX11, WindowType};
        let (w, h) = self.size();
        let mut attrs = Window::default_attributes()
            .with_title("LinusWare")
            .with_inner_size(LogicalSize::new(w as f64, h as f64))
            .with_min_inner_size(LogicalSize::new(w as f64, h as f64))
            .with_max_inner_size(LogicalSize::new(w as f64, h as f64))
            .with_resizable(false)
            .with_decorations(true)
            .with_visible(false)
            .with_window_icon(window_icon());
        attrs = WindowAttributesExtX11::with_name(attrs, "linusware", "linusware");
        attrs = WindowAttributesExtX11::with_x11_window_type(attrs, vec![WindowType::Dialog]);
        attrs = WindowAttributesExtWayland::with_name(attrs, "linusware", "linusware");

        let template = ConfigTemplateBuilder::new();
        let (window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(attrs))
            .build(el, template, |cfgs| cfgs.min_by_key(|c| c.num_samples()).unwrap())
            .map_err(|e| e.to_string())?;
        let window = Arc::new(window.ok_or("no window")?);
        let raw = window.window_handle().map_err(|e| e.to_string())?.as_raw();
        let wayland = matches!(raw, RawWindowHandle::Wayland(_));
        let display = config.display();
        let gl_attrs = ContextAttributesBuilder::new().with_profile(GlProfile::Core).with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3)))).build(Some(raw));
        let es_attrs = ContextAttributesBuilder::new().with_context_api(ContextApi::Gles(Some(Version::new(3, 0)))).build(Some(raw));
        let (ctx, gles) = unsafe {
            match display.create_context(&config, &gl_attrs) {
                Ok(c) => (c, false),
                Err(_) => (display.create_context(&config, &es_attrs).map_err(|e| e.to_string())?, true),
            }
        };
        let sattrs = window.build_surface_attributes(Default::default()).map_err(|e| e.to_string())?;
        let surface = unsafe { display.create_window_surface(&config, &sattrs).map_err(|e| e.to_string())? };
        let ctx = ctx.make_current(&surface).map_err(|e| e.to_string())?;
        let interval = if wayland { SwapInterval::DontWait } else { SwapInterval::Wait(NonZeroU32::new(1).unwrap()) };
        let _ = surface.set_swap_interval(&ctx, interval);
        let glc = unsafe {
            glow::Context::from_loader_function_cstr(|s| display.get_proc_address(s))
        };
        let scale = window.scale_factor() as f32;
        let mut gfx = Gfx::new(glc, gles, scale)?;
        gfx.oy = -TOP;
        self.gl = Some(Gl { window, surface, ctx, gfx, title_glow: [0.0; 4], logo_glow: [0.0; 4], glow_pad: (0.0, 0.0) });
        self.build_glow();
        Ok(())
    }

    fn build_glow(&mut self) {
        let Some(g) = self.gl.as_mut() else { return };
        let s = g.gfx.text.scale;
        let pad = (16.0 * s).ceil() as usize;
        let mut title = g.gfx.text.mask(Font::Bold, 34.0, "LinusWare", pad);
        blur(&mut title, (6.0 * s).round() as usize);
        let logo_px = (100.0 * s).round() as usize;
        let lpad = (22.0 * s).ceil() as usize;
        let src = crate::gfx::logo_rgba();
        let n = crate::gfx::LOGO_SIZE;
        let lw = logo_px + lpad * 2;
        let mut logo = crate::gfx::Mask { w: lw, h: lw, data: vec![0; lw * lw] };
        for y in 0..logo_px {
            for x in 0..logo_px {
                let sx = x * n / logo_px;
                let sy = y * n / logo_px;
                logo.data[(y + lpad) * lw + x + lpad] = src[(sy * n + sx) * 4 + 3];
            }
        }
        blur(&mut logo, (9.0 * s).round() as usize);
        let fw = title.w + logo.w;
        let fh = title.h.max(logo.h);
        let mut fx = crate::gfx::Mask { w: fw, h: fh, data: vec![0; fw * fh] };
        for y in 0..title.h {
            fx.data[y * fw..y * fw + title.w].copy_from_slice(&title.data[y * title.w..(y + 1) * title.w]);
        }
        for y in 0..logo.h {
            fx.data[y * fw + title.w..y * fw + title.w + logo.w].copy_from_slice(&logo.data[y * logo.w..(y + 1) * logo.w]);
        }
        g.gfx.upload_fx(&fx);
        let (fwf, fhf) = (fw as f32, fh as f32);
        g.title_glow = [0.0, 0.0, title.w as f32 / fwf, title.h as f32 / fhf];
        g.logo_glow = [title.w as f32 / fwf, 0.0, 1.0, logo.h as f32 / fhf];
        g.glow_pad = (pad as f32 / s, lpad as f32 / s);
    }

    fn installed(&self) -> bool {
        self.paths.installed()
    }

    fn busy(&self) -> bool {
        self.job != Job::Idle
    }

    fn note(&mut self, s: impl Into<String>, c: Rgba) {
        self.changelog = false;
        self.tone = Some((Tone(s.into(), c), Instant::now()));
    }

    // JOBS
    fn start_check(&mut self) {
        if !net::have_curl() {
            self.note("curl is missing, install it to download LinusWare", ERR);
            return;
        }
        let auto = self.cfg.auto_update && self.installed();
        if auto {
            self.start_update();
            return;
        }
        self.job = Job::Checking;
        let (paths, proxy) = (self.paths.clone(), self.proxy.clone());
        std::thread::spawn(move || {
            let _ = proxy.send_event(Msg::Checked(install::status(&paths, crate::EDITION)));
        });
    }

    fn start_update(&mut self) {
        if self.busy() {
            return;
        }
        if !net::have_curl() {
            self.note("curl is missing, install it to download LinusWare", ERR);
            return;
        }
        self.job = Job::Updating;
        self.tone = None;
        self.progress.set(install::PH_IDLE);
        let (paths, proxy, pr) = (self.paths.clone(), self.proxy.clone(), self.progress.clone());
        let fresh = !self.installed();
        std::thread::spawn(move || {
            let r = install::update_edition(&paths, &pr, fresh).and_then(|o| {
                install::refresh_scripts(&paths, |_| {});
                pr.set(install::PH_LAUNCHER);
                let me = install::self_update(&paths, &pr).unwrap_or(None);
                if paths.exe_installed() {
                    let _ = install::write_desktop_entry(&paths);
                }
                Ok((o, me))
            });
            let _ = proxy.send_event(Msg::Updated(r));
        });
    }

    fn ensure_bridge(&mut self) -> bool {
        if self.bridge.is_some() {
            return true;
        }
        let proxy = self.proxy.clone();
        match Bridge::start(move |r| {
            let _ = proxy.send_event(Msg::Ask(r));
        }) {
            Ok(b) => {
                self.bridge = Some(b);
                true
            }
            Err(e) => {
                self.note(format!("Could not open the password prompt: {e}"), ERR);
                false
            }
        }
    }

    fn start_launch(&mut self) {
        if self.busy() || !self.installed() || !self.ensure_bridge() {
            return;
        }
        self.job = Job::Launching;
        self.ask_tries = 0;
        self.note("Starting LinusWare\u{2026}", MUTED);
        let proxy = self.proxy.clone();
        let r = launch::start(&self.paths, self.bridge.as_ref().unwrap(), move |e| {
            let _ = proxy.send_event(Msg::Launch(e));
        });
        if let Err(e) = r {
            self.job = Job::Idle;
            self.note(format!("Could not run sudo: {e}"), ERR);
        }
    }

    fn start_delete(&mut self) {
        if self.busy() {
            return;
        }
        let plan = match remove::plan(&self.paths) {
            Ok(p) => p,
            Err(e) => {
                self.note(e, WARN);
                return;
            }
        };
        self.job = Job::Deleting;
        self.note("Deleting LinusWare\u{2026}", MUTED);
        let proxy = self.proxy.clone();
        let plan2 = remove::Plan { install: plan.install.clone(), entries: plan.entries.clone(), extra: plan.extra.clone() };
        self.root_rm = Some(plan);
        std::thread::spawn(move || {
            let r = remove::execute(&plan2);
            let _ = proxy.send_event(Msg::Deleted(Ok(r)));
        });
    }

    fn finish_delete(&mut self) {
        if let Some(p) = self.root_rm.take() {
            remove::finish(&p);
        }
        self.job = Job::Idle;
        self.remote = None;
        self.note("LinusWare removed", OK);
    }

    fn set_panel(&mut self, p: Panel) {
        if self.panel != p {
            self.prev_panel = self.panel;
            self.panel = p;
            self.panel_t = 0.0;
        }
    }

    fn asking(&self) -> bool {
        self.ask.is_some() || matches!(self.mode, Mode::Askpass(_))
    }

    fn submit_password(&mut self) {
        if let Mode::Askpass(_) = self.mode {
            println!("{}", self.password);
            self.password.clear();
            self.code = 0;
            self.quit();
            return;
        }
        if let Some(req) = self.ask.take() {
            let _ = req.reply.send(Some(std::mem::take(&mut self.password)));
            self.note("Checking password\u{2026}", MUTED);
        }
    }

    fn cancel_password(&mut self) {
        self.password.clear();
        if let Mode::Askpass(_) = self.mode {
            self.code = 1;
            self.quit();
            return;
        }
        if let Some(req) = self.ask.take() {
            let _ = req.reply.send(None);
        }
    }

    fn quit(&mut self) {
        self.fade = Some(Instant::now() - Duration::from_secs(10));
    }

    // INPUT
    fn hover_id(&self) -> Option<Id> {
        self.hits.iter().rev().find(|h| inside(h.r, self.mouse)).map(|h| h.id)
    }

    fn enabled(&self, id: Id) -> bool {
        self.hits.iter().any(|h| h.id == id && h.enabled)
    }

    fn click(&mut self, id: Id) {
        match id {
            Id::Gear => self.set_panel(if self.panel == Panel::Settings { Panel::Main } else { Panel::Settings }),
            Id::Help => self.set_panel(if self.panel == Panel::Help { Panel::Main } else { Panel::Help }),
            Id::External => self.start_launch(),
            Id::Update => self.start_update(),
            Id::TogAuto => {
                self.cfg.auto_update = !self.cfg.auto_update;
                self.cfg.save(&self.paths);
            }
            Id::TogClose => {
                self.cfg.close_after_launch = !self.cfg.close_after_launch;
                self.cfg.save(&self.paths);
            }
            Id::OpenFolder => launch::open(&self.paths.install.display().to_string()),
            Id::Discord => launch::open(DISCORD),
            Id::Backup => launch::open(DISCORD_BACKUP),
            Id::Changelog => self.set_panel(Panel::Help),
            Id::OpenLog => launch::open(&launch::log_path(&self.paths).display().to_string()),
            Id::PwOk => self.submit_password(),
            Id::PwCancel => self.cancel_password(),
            Id::Delete | Id::Field => {}
        }
    }

    fn key(&mut self, ev: &KeyEvent) {
        if ev.state != ElementState::Pressed {
            return;
        }
        let asking = self.asking();
        match &ev.logical_key {
            Key::Named(NamedKey::Escape) => {
                if asking {
                    self.cancel_password();
                } else if self.panel != Panel::Main {
                    self.set_panel(Panel::Main);
                }
            }
            Key::Named(NamedKey::Enter) => {
                if asking {
                    self.submit_password();
                } else if self.panel == Panel::Main && self.installed() && !self.busy() {
                    self.start_launch();
                }
            }
            Key::Named(NamedKey::Backspace) if asking => {
                self.password.pop();
                self.caret_t = Instant::now();
            }
            _ => {
                if asking {
                    if let Some(t) = &ev.text {
                        for c in t.chars().filter(|c| !c.is_control()) {
                            self.password.push(c);
                        }
                        self.caret_t = Instant::now();
                    }
                }
            }
        }
    }

    fn on_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Checked(r) => {
                self.job = Job::Idle;
                self.remote = Some(r);
            }
            Msg::Updated(r) => {
                self.job = Job::Idle;
                match r {
                    Ok((o, me)) => {
                        match &o {
                            Outcome::UpToDate(v, log) => {
                                self.remote = Some(Ok(Status { version: v.clone(), updated: true, log: log.clone() }));
                                self.done_flash = Some(Instant::now());
                            }
                            Outcome::Updated { to, log, .. } => {
                                self.remote = Some(Ok(Status { version: to.clone(), updated: true, log: log.clone() }));
                                self.note(format!("Updated to v{to}"), OK);
                                self.changelog = !log.is_empty();
                                self.done_flash = Some(Instant::now());
                            }
                            Outcome::NotReleased => {
                                self.remote = Some(Ok(Status { version: String::new(), updated: false, log: String::new() }));
                            }
                        }
                        if me.is_some() {
                            install::restart(&self.paths);
                        }
                    }
                    Err(e) => self.note(format!("Update failed: {e}"), ERR),
                }
            }
            Msg::Ask(req) => {
                self.ask_tries += 1;
                if self.ask_tries > 1 {
                    self.shake = Some(Instant::now());
                }
                self.password.clear();
                self.caret_t = Instant::now();
                self.ask = Some(req);
                self.set_panel(Panel::Main);
                if let Some(g) = &self.gl {
                    g.window.focus_window();
                }
            }
            Msg::Launch(ev) => match ev {
                launch::Event::Running => {
                    self.ask = None;
                    self.note("LinusWare is running", OK);
                }
                launch::Event::Stable => {
                    self.job = Job::Idle;
                    self.launched = true;
                    self.note("LinusWare is running", OK);
                    if self.cfg.close_after_launch {
                        self.fade = Some(Instant::now());
                    }
                }
                launch::Event::Exited { code, tail } => {
                    self.job = Job::Idle;
                    self.ask = None;
                    if code == 0 {
                        self.note("LinusWare closed", MUTED);
                    } else if self.ask_tries > 0 && (tail.contains("no password") || tail.contains("incorrect password") || tail.is_empty()) {
                        self.note(if tail.contains("incorrect") { "Wrong password" } else { "Cancelled" }, WARN);
                    } else {
                        self.note(if tail.is_empty() { format!("LinusWare exited ({code})") } else { tail }, ERR);
                    }
                }
            },
            Msg::Deleted(r) => match r {
                Ok(rep) if rep.failed.is_empty() => self.finish_delete(),
                Ok(rep) => {
                    if !self.ensure_bridge() {
                        self.job = Job::Idle;
                        return;
                    }
                    self.ask_tries = 0;
                    self.note("Some files belong to root, enter your password", WARN);
                    let proxy = self.proxy.clone();
                    let r = launch::sudo_rm(&self.paths, self.bridge.as_ref().unwrap(), &rep.failed, move |ok, t| {
                        let _ = proxy.send_event(Msg::RootRm(ok, t));
                    });
                    if let Err(e) = r {
                        self.job = Job::Idle;
                        self.note(format!("Could not delete: {e}"), ERR);
                    }
                }
                Err(e) => {
                    self.job = Job::Idle;
                    self.note(e, ERR);
                }
            },
            Msg::RootRm(ok, tail) => {
                self.ask = None;
                if ok {
                    self.finish_delete();
                } else {
                    self.job = Job::Idle;
                    self.note(if tail.is_empty() { "Delete cancelled".into() } else { tail }, WARN);
                }
            }
        }
    }

    fn remote_log(&self) -> String {
        match &self.remote {
            Some(Ok(s)) => s.log.clone(),
            _ => String::new(),
        }
    }

    // FRAME
    fn frame(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.dt = dt;
        self.last = now;
        let t = (now - self.t0).as_secs_f32();
        if self.mouse_in {
            if self.mouse_k < 0.02 {
                self.mouse_s = self.mouse;
            }
            ease(&mut self.mouse_s.0, self.mouse.0, dt, 13.0);
            ease(&mut self.mouse_s.1, self.mouse.1, dt, 13.0);
        }
        ease(&mut self.mouse_k, if self.mouse_in { 1.0 } else { 0.0 }, dt, 5.0);
        let motion = 1.0;

        if self.pressed == Some(Id::Delete) && self.hover_id() == Some(Id::Delete) && self.enabled(Id::Delete) {
            self.hold += dt / HOLD;
            if self.hold >= 1.0 {
                self.hold = 0.0;
                self.pressed = None;
                self.start_delete();
            }
        } else {
            ease(&mut self.hold, 0.0, dt, 9.0);
        }
        ease(&mut self.panel_t, 1.0, dt, 12.0);
        ease(&mut self.ask_t, if self.ask.is_some() { 1.0 } else { 0.0 }, dt, 14.0);

        let hovered = self.hover_id();
        let ids: Vec<Id> = self.hits.iter().map(|h| h.id).collect();
        for id in ids {
            let en = self.enabled(id);
            let a = self.anims.entry(id).or_default();
            if a.scale == 0.0 {
                a.scale = 1.0;
            }
            ease(&mut a.hover, if hovered == Some(id) && en { 1.0 } else { 0.0 }, dt, 14.0);
            ease(&mut a.press, if self.pressed == Some(id) && hovered == Some(id) && en { 1.0 } else { 0.0 }, dt, 22.0);
            ease(&mut a.on, if en { 1.0 } else { 0.0 }, dt, 8.0);
            let target = 1.0 + 0.025 * a.hover - 0.05 * a.press;
            let acc = 420.0 * (target - a.scale) - 22.0 * a.vel;
            a.vel += acc * dt;
            a.scale += a.vel * dt;
        }

        let fade = self.fade.map(|f| ((now - f).as_secs_f32() / 0.35).min(1.0)).unwrap_or(0.0);
        let mouse = (self.mouse_s.0, self.mouse_s.1 - TOP);
        self.hits.clear();
        let Some(mut g) = self.gl.take() else { return };
        match self.mode {
            Mode::Launcher => self.draw_launcher(&mut g, t, motion),
            Mode::Askpass(_) => self.draw_askpass(&mut g, t),
        }
        if fade > 0.0 {
            g.gfx.fill(0.0, TOP, W, H, 0.0, BG.a(fade));
        }
        let size = g.window.inner_size();
        let breath = 0.5 + 0.5 * (t * 1.3).sin() * motion;
        g.gfx.render(size.width, size.height, t, mouse, self.mouse_k, [250.0, 107.0 - TOP, 0.0, 0.7 + 0.3 * breath]);
        g.window.pre_present_notify();
        let _ = g.surface.swap_buffers(&g.ctx);
        if !self.shown {
            self.shown = true;
            g.window.set_visible(true);
        }
        if !self.trimmed && (now - self.t0).as_secs_f32() > 1.0 {
            self.trimmed = true;
            unsafe { malloc_trim(0) };
        }
        self.gl = Some(g);
        if fade >= 1.0 {
            self.code = if matches!(self.mode, Mode::Askpass(_)) { self.code } else { 0 };
            self.done = true;
        }
    }
}
