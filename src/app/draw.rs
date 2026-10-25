use super::*;
use winit::event::MouseScrollDelta;

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Primary,
    Secondary,
    Danger,
}

pub(super) fn window_icon() -> Option<Icon> {
    let src = crate::gfx::logo_rgba();
    let n = crate::gfx::LOGO_SIZE;
    let m = 64;
    let k = n / m;
    let mut out = vec![0u8; m * m * 4];
    for y in 0..m {
        for x in 0..m {
            let mut acc = [0u32; 4];
            for sy in 0..k {
                for sx in 0..k {
                    let i = ((y * k + sy) * n + x * k + sx) * 4;
                    for c in 0..4 {
                        acc[c] += src[i + c] as u32;
                    }
                }
            }
            let a = acc[3] / (k * k) as u32;
            let o = (y * m + x) * 4;
            for c in 0..3 {
                out[o + c] = if a > 0 { ((acc[c] / (k * k) as u32) * 255 / a).min(255) as u8 } else { 0 };
            }
            out[o + 3] = a as u8;
        }
    }
    Icon::from_rgba(out, m as u32, m as u32).ok()
}

fn scaled(r: [f32; 4], s: f32) -> [f32; 4] {
    let (cx, cy) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    [cx - r[2] * s * 0.5, cy - r[3] * s * 0.5, r[2] * s, r[3] * s]
}

impl App {
    fn hit(&mut self, id: Id, r: [f32; 4], enabled: bool) {
        self.hits.push(Hit { id, r, enabled });
    }

    fn st(&self, id: Id) -> (f32, f32, f32, f32, Option<(f32, f32, f32, f32)>) {
        match self.anims.get(&id) {
            Some(a) => {
                let rip = a.ripple.and_then(|(x, y, t0)| {
                    let p = t0.elapsed().as_secs_f32() / 0.55;
                    (p < 1.0).then(|| (x, y, 12.0 + 280.0 * p, 0.16 * (1.0 - p) * (1.0 - p)))
                });
                (a.hover, a.press, a.on, if a.scale == 0.0 { 1.0 } else { a.scale }, rip)
            }
            None => (0.0, 0.0, 0.0, 1.0, None),
        }
    }

    fn button(&mut self, g: &mut Gl, id: Id, r: [f32; 4], label: &str, kind: Btn, enabled: bool, alpha: f32, interactive: bool) {
        if alpha <= 0.01 {
            return;
        }
        if interactive {
            self.hit(id, r, enabled);
        }
        let (hov, press, on, sc, rip) = self.st(id);
        let rr = scaled(r, sc);
        let t = (Instant::now() - self.t0).as_secs_f32();
        let mut st = BoxStyle { radius: 7.0, ripple: rip, ..Default::default() };
        let text;
        match kind {
            Btn::Primary => {
                let base = GREEN.mix(GREEN_HI, hov).mix(Rgba::hex(0x00b45a), press);
                st.fill = Some(base.mix(Rgba::hex(0x1f3a2a), 1.0 - on).a(alpha));
                st.glow = Some((GLOW.a(0.2 * hov * alpha), 12.0));
                if on > 0.5 {
                    let cyc = t % 5.0;
                    if cyc < 1.3 {
                        st.shimmer = Some(cyc / 1.3 * 1.6 - 0.3);
                    }
                }
                text = DARK_TEXT.mix(Rgba::hex(0x6f8f7a), 1.0 - on).a(alpha);
            }
            Btn::Secondary => {
                st.fill = Some(BTN.mix(BTN_ON, on).mix(Rgba::hex(0x1b2b21), hov).a(alpha));
                st.border = Some((BTN_BORDER.mix(BTN_ON_BORDER, on).mix(GREEN, hov).a(alpha), 1.2));
                st.glow = Some((GREEN.a(0.12 * hov * alpha), 9.0));
                text = BTN_TEXT_OFF.mix(BTN_ON_TEXT, on).mix(WHITE, hov).a(alpha);
            }
            Btn::Danger => {
                st.fill = Some(BTN.mix(DEL, on).mix(Rgba::hex(0x2a1a1c), hov).a(alpha));
                st.border = Some((BTN_BORDER.mix(DEL_BORDER, on).mix(RED, hov.max(self.hold)).a(alpha), 1.2));
                st.glow = Some((RED.a((0.12 * hov + 0.15 * self.hold) * alpha), 9.0));
                if self.hold > 0.002 {
                    st.progress = Some((self.hold, RED.a(0.85 * alpha), 0.0));
                }
                text = BTN_TEXT_OFF.mix(DEL_TEXT, on).mix(Rgba::hex(0xffd6d7), hov).mix(WHITE, (self.hold * 3.0).min(1.0)).a(alpha);
            }
        }
        g.gfx.rect(rr[0], rr[1], rr[2], rr[3], st);
        let label = if kind == Btn::Danger && self.hold > 0.02 { "Hold to delete" } else { label };
        g.gfx.text(Font::Bold, 14.0, label, r[0] + r[2] * 0.5, r[1] + r[3] * 0.5 + 5.0, Align::Center, text);
    }

    fn circle_button(&mut self, g: &mut Gl, id: Id, cx: f32, cy: f32, icon: &str, alpha: f32) {
        if alpha <= 0.01 {
            return;
        }
        let r = [cx - 14.5, cy - 14.5, 29.0, 29.0];
        self.hit(id, r, true);
        let (hov, press, _, sc, _) = self.st(id);
        let rr = scaled(r, sc);
        g.gfx.rect(rr[0], rr[1], rr[2], rr[3], BoxStyle {
            fill: Some(CIRCLE.mix(CIRCLE_HI, hov).a(alpha)),
            radius: 15.0,
            glow: Some((GREEN.a(0.1 * hov * alpha), 7.0)),
            ..Default::default()
        });
        let _ = press;
        let size = 16.0;
        g.gfx.text(Font::Icon, size, icon, cx, cy + size * 0.36, Align::Center, MINT.mix(TITLE, hov).a(alpha));
    }

    fn toggle(&mut self, g: &mut Gl, id: Id, x: f32, y: f32, val: bool, alpha: f32) {
        let dt = self.dt;
        let a = self.anims.entry(id).or_default();
        if a.ripple.is_none() && a.vel == 0.0 && a.scale == 0.0 {
            a.on = if val { 1.0 } else { 0.0 };
        }
        let _ = dt;
        let (hov, _, _, _, _) = self.st(id);
        let k = self.anims.get(&id).map(|a| a.press).unwrap_or(0.0);
        let on = if val { 1.0f32 } else { 0.0 };
        let knob = self.knob(id, on);
        g.gfx.rect(x, y, 42.0, 24.0, BoxStyle {
            fill: Some(Rgba::hex(0x2a2f2c).mix(GREEN, knob).a(alpha)),
            radius: 12.0,
            glow: Some((GREEN.a(0.1 * hov * alpha), 7.0)),
            ..Default::default()
        });
        let kx = x + 3.0 + 18.0 * knob;
        let kw = 18.0 + 4.0 * k;
        g.gfx.fill(kx - (kw - 18.0) * knob, y + 3.0, kw, 18.0, 9.0, Rgba::hex(0xe8efe9).a(alpha));
    }

    fn knob(&mut self, id: Id, target: f32) -> f32 {
        let dt = self.dt;
        let e = self.knobs.entry(id).or_insert(target);
        ease(e, target, dt, 16.0);
        *e
    }

    fn field(&mut self, g: &mut Gl, r: [f32; 4], alpha: f32) {
        let now = Instant::now();
        let shake = self.shake.map(|s| (now - s).as_secs_f32()).filter(|e| *e < 0.45).map(|e| (e * 55.0).sin() * 7.0 * (1.0 - e / 0.45)).unwrap_or(0.0);
        let x = r[0] + shake;
        let wrong = self.ask_tries > 1 && self.password.is_empty();
        let edge = if wrong { RED } else { GREEN };
        g.gfx.rect(x, r[1], r[2], r[3], BoxStyle {
            fill: Some(Rgba::hex(0x0c1610).a(alpha)),
            border: Some((edge.a(alpha), 1.2)),
            radius: 7.0,
            glow: Some((edge.a(0.1 * alpha), 8.0)),
            ..Default::default()
        });
        let mid = r[1] + r[3] * 0.5;
        g.gfx.text(Font::Icon, 13.0, ICON_LOCK, x + 16.0, mid + 5.0, Align::Center, Rgba::hex(0x4d8f66).a(alpha));
        let n = self.password.chars().count();
        let shown = n.min(((r[2] - 50.0) / 12.0) as usize);
        for i in 0..shown {
            g.gfx.fill(x + 34.0 + i as f32 * 12.0, mid - 3.5, 7.0, 7.0, 3.5, BTN_ON_TEXT.a(alpha));
        }
        if n == 0 {
            g.gfx.text(Font::Medium, 13.0, "Password", x + 34.0, mid + 4.5, Align::Left, SUBTLE.a(alpha));
        }
        if (now - self.caret_t).as_secs_f32() % 1.0 < 0.55 {
            let cx = x + 33.0 + shown as f32 * 12.0 - if n == 0 { 3.0 } else { 0.0 };
            g.gfx.fill(cx, mid - 9.0, 1.5, 18.0, 0.5, GREEN.a(alpha));
        }
        self.hit(Id::Field, r, true);
    }

    fn status_line(&mut self) -> (String, Rgba, bool) {
        let ph = self.progress.phase.load(Ordering::Relaxed);
        match self.job {
            Job::Checking => return ("Checking for updates\u{2026}".into(), MUTED, true),
            Job::Updating => {
                let s = match ph {
                    install::PH_VERIFY => "Verifying download\u{2026}",
                    install::PH_INSTALL => "Installing\u{2026}",
                    install::PH_LAUNCHER => "Updating the launcher\u{2026}",
                    install::PH_DOWNLOAD => "Downloading LinusWare\u{2026}",
                    _ => "Checking for updates\u{2026}",
                };
                return (s.into(), MUTED, true);
            }
            _ => {}
        }
        if let Some((Tone(s, c), at)) = &self.tone {
            let sticky = *c == ERR || *c == WARN || self.job != Job::Idle || self.changelog;
            if sticky || at.elapsed() < Duration::from_secs(8) {
                return (s.clone(), *c, self.job == Job::Launching || self.job == Job::Deleting);
            }
        }
        let cur = self.paths.installed_version();
        match (&self.remote, cur) {
            (Some(Ok(st)), _) if !st.updated => ("External isn't released yet".into(), WARN, false),
            (Some(Ok(st)), Some(v)) if st.version != v => (format!("Update available \u{b7} v{v} \u{2192} v{}", st.version), WARN, false),
            (_, Some(v)) if self.installed() => (format!("External v{v} \u{b7} up to date"), MUTED, false),
            (Some(Ok(st)), None) => (format!("Not installed \u{b7} v{} available", st.version), MUTED, false),
            (Some(Err(_)), None) => ("Couldn't reach git.catapis.uk".into(), ERR, false),
            _ => ("Not installed".into(), MUTED, false),
        }
    }

    // LAUNCHER
    pub(super) fn draw_launcher(&mut self, g: &mut Gl, t: f32, motion: f32) {
        let it = (Instant::now() - self.intro).as_secs_f32();
        let k = self.panel_t;
        if k < 0.995 {
            self.draw_panel(g, self.prev_panel, 1.0 - k, false, t, motion, it);
        }
        self.draw_panel(g, self.panel, k, k > 0.5, t, motion, it);

        let main = matches!(self.panel, Panel::Main | Panel::Settings | Panel::Help | Panel::Changelog);
        if main {
            let gear = if self.panel == Panel::Settings { ICON_X } else { ICON_GEAR };
            let help = if matches!(self.panel, Panel::Help | Panel::Changelog) { ICON_X } else { ICON_HELP };
            let ia = (it * 3.0 - 1.2).clamp(0.0, 1.0);
            self.circle_button(g, Id::Gear, 462.0, 70.0, gear, ia);
            self.circle_button(g, Id::Help, 462.0, 344.0, help, ia);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_panel(&mut self, g: &mut Gl, p: Panel, alpha: f32, live: bool, t: f32, motion: f32, it: f32) {
        if alpha <= 0.01 {
            return;
        }
        let dy = (1.0 - alpha) * 10.0;
        match p {
            Panel::Main => self.draw_main(g, alpha, live, t, motion, it),
            Panel::Settings => self.draw_settings(g, alpha, live, dy),
            Panel::Help => self.draw_help(g, alpha, live, dy),
            Panel::Changelog => self.draw_changelog(g, alpha, live, dy),
        }
    }

    fn draw_main(&mut self, g: &mut Gl, alpha: f32, live: bool, t: f32, motion: f32, it: f32) {
        let stage = |d: f32| ((it - d) / 0.45).clamp(0.0, 1.0);
        let pop = |x: f32| 1.0 - (1.0 - x).powi(3);
        let s0 = pop(stage(0.0));
        let float = (t * 1.6).sin() * 2.2 * motion;
        let breath = 0.5 + 0.5 * (t * 1.3).sin() * motion;

        let ls = 0.86 + 0.14 * s0;
        let (lx, ly, lw) = (250.0 - 50.0 * ls, 57.0 + float + (1.0 - s0) * 8.0, 100.0 * ls);
        let (pad, lpad) = g.glow_pad;
        g.gfx.fx(200.0 - lpad, 57.0 + float - lpad, g.logo_glow, GLOW.a((0.08 + 0.05 * breath) * alpha * s0));
        g.gfx.image(lx, ly, lw, lw, alpha * s0);

        let s1 = pop(stage(0.12));
        let tw = g.gfx.text.measure(Font::Bold, 34.0, "LinusWare");
        let asc = g.gfx.text.ascent(Font::Bold, 34.0);
        let ty = 194.0 + (1.0 - s1) * 8.0;
        g.gfx.fx(250.0 - tw * 0.5 - pad, ty - asc - pad, g.title_glow, GLOW.a((0.26 + 0.1 * breath) * alpha * s1));
        g.gfx.text(Font::Bold, 34.0, "LinusWare", 250.0, ty, Align::Center, TITLE.a(alpha * s1));

        let s2 = pop(stage(0.24));
        let (line, color, spin) = self.status_line();
        let line = g.gfx.text.ellipsize(Font::Medium, 13.0, &line, 380.0);
        let lw2 = g.gfx.text.measure(Font::Medium, 13.0, &line);
        let sa = alpha * s2 * (1.0 - self.ask_t * 0.0);
        let link = self.changelog && color == OK && line.starts_with("Updated to");
        if link {
            let head = format!("{line} \u{b7} ");
            let hw = g.gfx.text.measure(Font::Medium, 13.0, &head);
            let lk = "See changelog";
            let kw = g.gfx.text.measure(Font::Bold, 13.0, lk);
            let x0 = 250.0 - (hw + kw) * 0.5;
            g.gfx.text(Font::Medium, 13.0, &head, x0, 221.0, Align::Left, color.a(sa));
            let r = [x0 + hw - 2.0, 207.0, kw + 4.0, 20.0];
            if live {
                self.hit(Id::Changelog, r, true);
            }
            let (hov, ..) = self.st(Id::Changelog);
            g.gfx.text(Font::Bold, 13.0, lk, x0 + hw, 221.0, Align::Left, TITLE.mix(WHITE, hov).a(sa));
            g.gfx.fill(x0 + hw, 223.5, kw * hov, 1.0, 0.5, TITLE.mix(WHITE, hov).a(sa));
        } else if spin {
            g.gfx.spinner(250.0 - lw2 * 0.5 - 12.0, 216.5, 5.5, 1.6, color.a(sa), t * 5.0);
            g.gfx.text(Font::Medium, 13.0, &line, 250.0 + 7.0, 221.0, Align::Center, color.a(sa));
        } else {
            g.gfx.text(Font::Medium, 13.0, &line, 250.0, 221.0, Align::Center, color.a(sa));
        }

        let ba = alpha * (1.0 - self.ask_t);
        let by = self.ask_t * 12.0;
        let s3 = pop(stage(0.34));
        let s4 = pop(stage(0.42));
        let installed = self.installed();
        let idle = !self.busy() && self.ask.is_none();
        let blive = live && self.ask_t < 0.5;
        self.button(g, Id::External, [109.0, 237.0 + by + (1.0 - s3) * 10.0, 132.0, 46.0], "External", Btn::Secondary, installed && idle, ba * s3, blive);
        self.button(g, Id::Delete, [259.0, 237.0 + by + (1.0 - s3) * 10.0, 132.0, 46.0], "Delete LinusWare", Btn::Danger, installed && (idle || self.job == Job::Idle), ba * s3, blive);

        let ur = [109.0, 301.0 + by + (1.0 - s4) * 10.0, 282.0, 46.0];
        if self.job == Job::Updating {
            let got = self.progress.got.load(Ordering::Relaxed) as f32;
            let total = self.progress.total.load(Ordering::Relaxed) as f32;
            let ph = self.progress.phase.load(Ordering::Relaxed);
            let frac = if ph == install::PH_DOWNLOAD && total > 0.0 { (got / total).min(1.0) } else if ph >= install::PH_VERIFY { 1.0 } else { 0.02 };
            let shown = self.knob(Id::Update, frac);
            g.gfx.rect(ur[0], ur[1], ur[2], ur[3], BoxStyle {
                fill: Some(Rgba::hex(0x0b3d23).a(ba * s4)),
                radius: 7.0,
                progress: Some((shown.max(0.02), Rgba::hex(0x00a352).a(ba * s4), t)),
                glow: Some((GLOW.a(0.1 * ba), 10.0)),
                ..Default::default()
            });
            let label = if ph == install::PH_DOWNLOAD && total > 0.0 {
                format!("{:.1} / {:.1} MB", got / 1_048_576.0, total / 1_048_576.0)
            } else if ph == install::PH_VERIFY {
                "Verifying\u{2026}".into()
            } else if ph >= install::PH_INSTALL {
                "Installing\u{2026}".into()
            } else {
                "Connecting\u{2026}".into()
            };
            g.gfx.text(Font::Bold, 14.0, &label, 250.0, ur[1] + 28.0, Align::Center, WHITE.a(ba * s4));
            if blive {
                self.hit(Id::Update, ur, false);
            }
        } else {
            let flash = self.done_flash.map(|f| f.elapsed().as_secs_f32()).filter(|e| *e < 2.2);
            let label = if flash.is_some() {
                "Up to date"
            } else if installed {
                "Check for updates"
            } else {
                "Install LinusWare"
            };
            let enabled = idle;
            self.button(g, Id::Update, ur, label, Btn::Primary, enabled, ba * s4, blive);
            if let Some(e) = flash {
                let p = (e / 0.35).min(1.0);
                let w = g.gfx.text.measure(Font::Bold, 14.0, label);
                g.gfx.text(Font::Icon, 14.0 * (0.6 + 0.4 * p), "\u{f00c}", 250.0 - w * 0.5 - 13.0, ur[1] + 28.0, Align::Center, DARK_TEXT.a(ba * s4 * p));
            }
        }

        if self.ask_t > 0.01 {
            let a = alpha * self.ask_t;
            let oy = (1.0 - self.ask_t) * 12.0;
            let (label, c) = if self.ask_tries > 1 { ("Wrong password, try again", ERR) } else { ("Root password needed", BODY) };
            g.gfx.text(Font::Medium, 13.0, &label, 250.0, 247.0 + oy, Align::Center, c.a(a));
            self.field(g, [109.0, 256.0 + oy, 282.0, 40.0], a);
            let live2 = live && self.ask_t > 0.5;
            self.button(g, Id::PwCancel, [109.0, 306.0 + oy, 132.0, 40.0], "Cancel", Btn::Secondary, true, a, live2);
            self.button(g, Id::PwOk, [259.0, 306.0 + oy, 132.0, 40.0], "Unlock", Btn::Primary, !self.password.is_empty(), a, live2);
        }
    }

    fn heading(&mut self, g: &mut Gl, s: &str, y: f32, alpha: f32) {
        g.gfx.text(Font::Bold, 20.0, s, 40.0, y, Align::Left, TITLE.a(alpha));
    }

    fn draw_settings(&mut self, g: &mut Gl, alpha: f32, live: bool, dy: f32) {
        g.gfx.fill(0.0, TOP, W, H, 0.0, BG.a(0.86 * alpha));
        self.heading(g, "Settings", 78.0 + dy, alpha);
        let rows = [
            (Id::TogAuto, "Update on start", "Check for and install updates when the launcher opens", self.cfg.auto_update),
            (Id::TogClose, "Close after launch", "Hide the launcher once LinusWare is running", self.cfg.close_after_launch),
        ];
        for (i, (id, label, desc, val)) in rows.into_iter().enumerate() {
            let y = 98.0 + i as f32 * 54.0 + dy;
            let (hov, ..) = self.st(id);
            g.gfx.fill(32.0, y, 436.0, 48.0, 8.0, Rgba::hex(0xffffff).a(0.035 * hov * alpha));
            g.gfx.text(Font::Bold, 14.0, label, 44.0, y + 21.0, Align::Left, BTN_ON_TEXT.a(alpha));
            g.gfx.text(Font::Medium, 12.0, desc, 44.0, y + 38.0, Align::Left, SUBTLE.a(alpha));
            self.toggle(g, id, 414.0, y + 12.0, val, alpha);
            if live {
                self.hit(id, [32.0, y, 436.0, 48.0], true);
            }
        }
        let y = 212.0 + dy;
        g.gfx.fill(40.0, y - 6.0, 420.0, 1.0, 0.0, Rgba::hex(0xffffff).a(0.06 * alpha));
        g.gfx.text(Font::Bold, 14.0, "Install folder", 44.0, y + 21.0, Align::Left, BTN_ON_TEXT.a(alpha));
        let path = self.paths.pretty(&self.paths.install);
        let path = g.gfx.text.ellipsize(Font::Medium, 12.0, &path, 300.0);
        g.gfx.text(Font::Medium, 12.0, &path, 44.0, y + 38.0, Align::Left, SUBTLE.a(alpha));
        self.button(g, Id::OpenFolder, [384.0, y + 9.0, 76.0, 32.0], "Open", Btn::Secondary, self.paths.install.is_dir(), alpha, live);
        g.gfx.text(Font::Medium, 11.0, &format!("Launcher v{}", crate::VERSION), 40.0, 362.0 + dy, Align::Left, MUTED.a(alpha));
    }

    fn draw_help(&mut self, g: &mut Gl, alpha: f32, live: bool, dy: f32) {
        g.gfx.fill(0.0, TOP, W, H, 0.0, BG.a(0.86 * alpha));
        self.heading(g, "Help", 78.0 + dy, alpha);
        let para = "Check for updates downloads the newest build from git.catapis.uk and checks it before installing.";
        let lines = g.gfx.text.wrap(Font::Medium, 15.0, para, 420.0);
        let mut y = 110.0 + dy;
        for l in &lines {
            g.gfx.text(Font::Medium, 15.0, l, 40.0, y, Align::Left, BODY.a(alpha));
            y += 22.0;
        }
        y += 16.0;
        let ext = self.paths.installed_version().map(|v| format!("v{v}")).unwrap_or_else(|| "not installed".into());
        let latest = match &self.remote {
            Some(Ok(s)) if s.updated => format!("  \u{b7}  latest v{}", s.version),
            _ => String::new(),
        };
        for (k, v) in [("Launcher", format!("v{}", crate::VERSION)), ("External", format!("{ext}{latest}"))] {
            g.gfx.text(Font::Bold, 15.0, k, 40.0, y, Align::Left, BTN_ON_TEXT.a(alpha));
            g.gfx.text(Font::Medium, 15.0, &v, 140.0, y, Align::Left, SUBTLE.a(alpha));
            y += 24.0;
        }
        let log = self.remote_log();
        if !log.is_empty() {
            y += 12.0;
            g.gfx.text(Font::Bold, 15.0, "What's new", 40.0, y, Align::Left, BTN_ON_TEXT.a(alpha));
            let title = log_title(&log);
            let title = g.gfx.text.ellipsize(Font::Medium, 15.0, &title, 180.0);
            g.gfx.text(Font::Medium, 15.0, &title, 140.0, y, Align::Left, SUBTLE.a(alpha));
            self.button(g, Id::Changelog, [330.0, y - 21.0, 130.0, 30.0], "See changelog", Btn::Secondary, true, alpha, live);
        }
        let by = 304.0 + dy;
        self.button(g, Id::Discord, [40.0, by, 120.0, 36.0], "Discord", Btn::Secondary, true, alpha, live);
        self.button(g, Id::Backup, [170.0, by, 140.0, 36.0], "Backup Discord", Btn::Secondary, true, alpha, live);
        let has_log = launch::log_path(&self.paths).exists();
        self.button(g, Id::OpenLog, [320.0, by, 104.0, 36.0], "Open log", Btn::Secondary, has_log, alpha, live);
    }

    fn draw_changelog(&mut self, g: &mut Gl, alpha: f32, live: bool, dy: f32) {
        g.gfx.fill(0.0, TOP, W, H, 0.0, BG.a(0.86 * alpha));
        self.heading(g, "What's new", 78.0 + dy, alpha);
        if let Some(Ok(s)) = &self.remote {
            let hw = g.gfx.text.measure(Font::Bold, 20.0, "What's new");
            g.gfx.text(Font::Medium, 14.0, &format!("v{}", s.version), 40.0 + hw + 10.0, 78.0 + dy, Align::Left, SUBTLE.a(alpha));
        }
        let (vx, vy, vw, vh) = (40.0, 94.0 + dy, 400.0, 262.0);
        let log = self.remote_log();
        let mut items: Vec<(Rgba, Option<Rgba>, Font, String)> = Vec::new();
        for raw in log.lines() {
            let line = raw.trim();
            if line.is_empty() {
                items.push((BODY, None, Font::Medium, String::new()));
            } else if line.starts_with('=') && line.ends_with('=') {
                items.push((TITLE, None, Font::Bold, line.trim_matches('=').trim().to_string()));
            } else {
                let dot = match line.split_whitespace().next().unwrap_or("") {
                    "Added" => Some(GREEN),
                    "Fixed" => Some(WARN),
                    "Removed" => Some(RED),
                    _ => Some(SUBTLE),
                };
                for (i, l) in g.gfx.text.wrap(Font::Medium, 14.0, line, vw - 24.0).into_iter().enumerate() {
                    items.push((BODY, if i == 0 { dot } else { None }, Font::Medium, l));
                }
            }
        }
        let h: f32 = items.iter().map(|(_, _, f, t)| if t.is_empty() { 8.0 } else if *f == Font::Bold { 28.0 } else { 21.0 }).sum();
        self.log_max = (h - vh + 6.0).max(0.0);
        self.log_target = self.log_target.clamp(0.0, self.log_max);
        ease(&mut self.log_scroll, self.log_target, self.dt, 16.0);
        g.gfx.clip = Some([vx - 4.0, vy, vw + 8.0, vh]);
        let mut y = vy + 16.0 - self.log_scroll;
        for (c, dot, f, t) in &items {
            if t.is_empty() {
                y += 8.0;
                continue;
            }
            if y > vy - 30.0 && y < vy + vh + 30.0 {
                if *f == Font::Bold {
                    g.gfx.text(Font::Bold, 15.0, t, vx, y + 2.0, Align::Left, c.a(alpha));
                } else {
                    if let Some(d) = dot {
                        g.gfx.fill(vx + 2.0, y - 8.0, 6.0, 6.0, 3.0, d.a(alpha));
                    }
                    g.gfx.text(Font::Medium, 14.0, t, vx + 18.0, y, Align::Left, c.a(alpha));
                }
            }
            y += if *f == Font::Bold { 28.0 } else { 21.0 };
        }
        g.gfx.clip = None;
        if self.log_max > 0.0 {
            let th = (vh * vh / (vh + self.log_max)).max(24.0);
            let ty = vy + (vh - th) * (self.log_scroll / self.log_max).clamp(0.0, 1.0);
            g.gfx.fill(vx + vw + 14.0, vy, 3.0, vh, 1.5, Rgba::hex(0xffffff).a(0.05 * alpha));
            g.gfx.fill(vx + vw + 14.0, ty, 3.0, th, 1.5, MINT.a(0.55 * alpha));
        }
        let _ = live;
    }

    // ASKPASS WINDOW
    pub(super) fn draw_askpass(&mut self, g: &mut Gl, _t: f32) {
        let Mode::Askpass(prompt) = &self.mode else { return };
        let user = askpass::user_from_prompt(prompt);
        let label = match user {
            Some(u) => format!("Password for {u}"),
            None => prompt.trim().trim_end_matches(':').to_string(),
        };
        g.gfx.image(24.0, 44.0, 36.0, 36.0, 1.0);
        g.gfx.text(Font::Bold, 19.0, "LinusWare", 70.0, 69.0, Align::Left, TITLE);
        let label = g.gfx.text.ellipsize(Font::Medium, 13.0, &label, ASK_W - 48.0);
        g.gfx.text(Font::Medium, 13.0, &label, 24.0, 106.0, Align::Left, BODY);
        self.field(g, [24.0, 114.0, ASK_W - 48.0, 38.0], 1.0);
        self.button(g, Id::PwCancel, [24.0, 162.0, 158.0, 38.0], "Cancel", Btn::Secondary, true, 1.0, true);
        self.button(g, Id::PwOk, [198.0, 162.0, 158.0, 38.0], "Unlock", Btn::Primary, !self.password.is_empty(), 1.0, true);
    }

    fn cursor(&self) -> CursorIcon {
        match self.hits.iter().rev().find(|h| inside(h.r, self.mouse)) {
            Some(h) if h.id == Id::Field => CursorIcon::Text,
            Some(h) if h.enabled => CursorIcon::Pointer,
            _ => CursorIcon::Default,
        }
    }
}

// EVENTS
impl ApplicationHandler<Msg> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gl.is_some() {
            return;
        }
        if let Err(e) = self.create(el) {
            self.err = Some(e);
            el.exit();
            return;
        }
        self.intro = Instant::now();
        if let Mode::Launcher = self.mode {
            if self.paths.exe_installed() {
                let _ = install::write_desktop_entry(&self.paths);
            }
            self.start_check();
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, msg: Msg) {
        self.on_msg(msg);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => {
                self.cancel_password();
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                self.frame();
                if self.done {
                    el.exit();
                }
            }
            WindowEvent::Resized(sz) => {
                if let Some(g) = &self.gl {
                    if let (Some(w), Some(h)) = (NonZeroU32::new(sz.width), NonZeroU32::new(sz.height)) {
                        g.surface.resize(&g.ctx, w, h);
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(g) = self.gl.as_mut() {
                    g.gfx.set_scale(scale_factor as f32);
                }
                self.build_glow();
            }
            WindowEvent::Focused(f) => self.focused = f,
            WindowEvent::Occluded(o) => self.hidden = o,
            WindowEvent::CursorMoved { position, .. } => {
                let s = self.gl.as_ref().map(|g| g.gfx.text.scale).unwrap_or(1.0);
                self.mouse = (position.x as f32 / s, position.y as f32 / s + TOP);
                self.mouse_in = true;
                if let Some(g) = &self.gl {
                    g.window.set_cursor(self.cursor());
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.mouse_in = false;
                self.mouse = (-1e4, -1e4);
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => {
                    let hov = self.hover_id();
                    match hov {
                        Some(id) if self.enabled(id) => {
                            self.pressed = Some(id);
                            let r = self.hits.iter().rev().find(|h| h.id == id).map(|h| h.r).unwrap_or([0.0; 4]);
                            let rel = (self.mouse.0 - r[0] - r[2] * 0.5, self.mouse.1 - r[1] - r[3] * 0.5);
                            self.anims.entry(id).or_default().ripple = Some((rel.0, rel.1, Instant::now()));
                        }
                        _ => {}
                    }
                }
                ElementState::Released => {
                    if let Some(p) = self.pressed.take() {
                        if self.hover_id() == Some(p) {
                            self.click(p);
                        }
                    }
                }
            },
            WindowEvent::KeyboardInput { event, .. } => self.key(&event),
            WindowEvent::MouseWheel { delta, .. } => {
                if self.panel == Panel::Changelog {
                    let s = self.gl.as_ref().map(|g| g.gfx.text.scale).unwrap_or(1.0);
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y * 44.0,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / s,
                    };
                    self.log_target = (self.log_target - dy).clamp(0.0, self.log_max);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.done {
            el.exit();
            return;
        }
        let fps = if self.focused { 60.0 } else { 30.0 };
        let now = Instant::now();
        if now >= self.next {
            if let Some(g) = &self.gl {
                if !self.hidden {
                    g.window.request_redraw();
                }
            }
            self.next = now + Duration::from_secs_f32(1.0 / fps);
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}

fn log_title(log: &str) -> String {
    let first = log.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    first.trim_matches('=').trim().to_string()
}
