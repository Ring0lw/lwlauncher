use crate::LinusWare;
use super::*;
use winit::event::MouseScrollDelta;

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Primary,
    Secondary,
    Danger,
}

pub(super) fn WindowIcon() -> Option<Icon> {
    let src = LinusWare::gfx::LogoRgba();
    let n = crate::LinusWare::gfx::LOGO_SIZE;
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

fn Scrim(g: &mut Gl, alpha: f32) {
    let (ox, oy) = (g.gfx.ox, g.gfx.oy);
    g.gfx.ox = 0.0;
    g.gfx.oy = -TOP;
    g.gfx.Fill(0.0, TOP, W, H, 0.0, BG.A(0.86 * alpha));
    g.gfx.ox = ox;
    g.gfx.oy = oy;
}

fn Scaled(r: [f32; 4], s: f32) -> [f32; 4] {
    let (cx, cy) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    [cx - r[2] * s * 0.5, cy - r[3] * s * 0.5, r[2] * s, r[3] * s]
}

impl App {
    fn Hit(&mut self, id: Id, r: [f32; 4], enabled: bool) {
        self.hits.push(Hit { id, r: [r[0] + self.hit_ox, r[1] + self.hit_oy, r[2], r[3]], enabled });
    }

    fn St(&self, id: Id) -> (f32, f32, f32, f32, Option<(f32, f32, f32, f32)>) {
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

    fn Button(&mut self, g: &mut Gl, id: Id, r: [f32; 4], label: &str, kind: Btn, enabled: bool, alpha: f32, interactive: bool) {
        if alpha <= 0.01 {
            return;
        }
        if interactive {
            self.Hit(id, r, enabled);
        }
        let (hov, press, on, sc, rip) = self.St(id);
        let rr = LinusWare::app::draw::Scaled(r, sc);
        let t = (Instant::now() - self.t0).as_secs_f32();
        let mut st = BoxStyle { radius: 7.0, ripple: rip, ..Default::default() };
        let text;
        match kind {
            Btn::Primary => {
                let base = GREEN.Mix(GREEN_HI, hov).Mix(LinusWare::gfx::Rgba::Hex(0x00b45a), press);
                st.fill = Some(base.Mix(LinusWare::gfx::Rgba::Hex(0x1f3a2a), 1.0 - on).A(alpha));
                st.glow = Some((GLOW.A(0.2 * hov * alpha), 12.0));
                if on > 0.5 {
                    let cyc = t % 5.0;
                    if cyc < 1.3 {
                        st.shimmer = Some(cyc / 1.3 * 1.6 - 0.3);
                    }
                }
                text = DARK_TEXT.Mix(LinusWare::gfx::Rgba::Hex(0x6f8f7a), 1.0 - on).A(alpha);
            }
            Btn::Secondary => {
                st.fill = Some(BTN.Mix(BTN_ON, on).Mix(LinusWare::gfx::Rgba::Hex(0x1b2b21), hov).A(alpha));
                st.border = Some((BTN_BORDER.Mix(BTN_ON_BORDER, on).Mix(GREEN, hov).A(alpha), 1.2));
                st.glow = Some((GREEN.A(0.12 * hov * alpha), 9.0));
                text = BTN_TEXT_OFF.Mix(BTN_ON_TEXT, on).Mix(WHITE, hov).A(alpha);
            }
            Btn::Danger => {
                st.fill = Some(BTN.Mix(DEL, on).Mix(LinusWare::gfx::Rgba::Hex(0x2a1a1c), hov).A(alpha));
                st.border = Some((BTN_BORDER.Mix(DEL_BORDER, on).Mix(RED, hov.max(self.hold)).A(alpha), 1.2));
                st.glow = Some((RED.A((0.12 * hov + 0.15 * self.hold) * alpha), 9.0));
                if self.hold > 0.002 {
                    st.progress = Some((self.hold, RED.A(0.85 * alpha), 0.0));
                }
                text = BTN_TEXT_OFF.Mix(DEL_TEXT, on).Mix(LinusWare::gfx::Rgba::Hex(0xffd6d7), hov).Mix(WHITE, (self.hold * 3.0).min(1.0)).A(alpha);
            }
        }
        g.gfx.Rect(rr[0], rr[1], rr[2], rr[3], st);
        let label = if kind == Btn::Danger && self.hold > 0.02 { "Hold to delete" } else { label };
        g.gfx.Text(Font::Bold, 14.0, label, r[0] + r[2] * 0.5, r[1] + r[3] * 0.5 + 5.0, Align::Center, text);
    }

    fn CircleButton(&mut self, g: &mut Gl, id: Id, cx: f32, cy: f32, icon: &str, alpha: f32) {
        if alpha <= 0.01 {
            return;
        }
        let r = [cx - 14.5, cy - 14.5, 29.0, 29.0];
        self.Hit(id, r, true);
        let (hov, press, _, sc, _) = self.St(id);
        let rr = LinusWare::app::draw::Scaled(r, sc);
        g.gfx.Rect(rr[0], rr[1], rr[2], rr[3], BoxStyle {
            fill: Some(CIRCLE.Mix(CIRCLE_HI, hov).A(alpha)),
            radius: 15.0,
            glow: Some((GREEN.A(0.1 * hov * alpha), 7.0)),
            ..Default::default()
        });
        let _ = press;
        let size = 16.0;
        g.gfx.Text(Font::Icon, size, icon, cx, cy + size * 0.36, Align::Center, MINT.Mix(TITLE, hov).A(alpha));
    }

    fn Toggle(&mut self, g: &mut Gl, id: Id, x: f32, y: f32, val: bool, alpha: f32) {
        let dt = self.dt;
        let a = self.anims.entry(id).or_default();
        if a.ripple.is_none() && a.vel == 0.0 && a.scale == 0.0 {
            a.on = if val { 1.0 } else { 0.0 };
        }
        let _ = dt;
        let (hov, _, _, _, _) = self.St(id);
        let k = self.anims.get(&id).map(|a| a.press).unwrap_or(0.0);
        let on = if val { 1.0f32 } else { 0.0 };
        let knob = self.Knob(id, on);
        g.gfx.Rect(x, y, 42.0, 24.0, BoxStyle {
            fill: Some(LinusWare::gfx::Rgba::Hex(0x2a2f2c).Mix(GREEN, knob).A(alpha)),
            radius: 12.0,
            glow: Some((GREEN.A(0.1 * hov * alpha), 7.0)),
            ..Default::default()
        });
        let kx = x + 3.0 + 18.0 * knob;
        let kw = 18.0 + 4.0 * k;
        g.gfx.Fill(kx - (kw - 18.0) * knob, y + 3.0, kw, 18.0, 9.0, LinusWare::gfx::Rgba::Hex(0xe8efe9).A(alpha));
    }

    fn Knob(&mut self, id: Id, target: f32) -> f32 {
        let dt = self.dt;
        let e = self.knobs.entry(id).or_insert(target);
        LinusWare::app::Ease(e, target, dt, 16.0);
        *e
    }

    fn Field(&mut self, g: &mut Gl, r: [f32; 4], alpha: f32) {
        let now = Instant::now();
        let shake = self.shake.map(|s| (now - s).as_secs_f32()).filter(|e| *e < 0.45).map(|e| (e * 55.0).sin() * 7.0 * (1.0 - e / 0.45)).unwrap_or(0.0);
        let x = r[0] + shake;
        let wrong = self.ask_tries > 1 && self.password.is_empty();
        let edge = if wrong { RED } else { GREEN };
        g.gfx.Rect(x, r[1], r[2], r[3], BoxStyle {
            fill: Some(LinusWare::gfx::Rgba::Hex(0x0c1610).A(alpha)),
            border: Some((edge.A(alpha), 1.2)),
            radius: 7.0,
            glow: Some((edge.A(0.1 * alpha), 8.0)),
            ..Default::default()
        });
        let mid = r[1] + r[3] * 0.5;
        g.gfx.Text(Font::Icon, 13.0, ICON_LOCK, x + 16.0, mid + 5.0, Align::Center, LinusWare::gfx::Rgba::Hex(0x4d8f66).A(alpha));
        let n = self.password.chars().count();
        let shown = n.min(((r[2] - 50.0) / 12.0) as usize);
        for i in 0..shown {
            g.gfx.Fill(x + 34.0 + i as f32 * 12.0, mid - 3.5, 7.0, 7.0, 3.5, BTN_ON_TEXT.A(alpha));
        }
        if n == 0 {
            g.gfx.Text(Font::Medium, 13.0, "Password", x + 34.0, mid + 4.5, Align::Left, SUBTLE.A(alpha));
        }
        if (now - self.caret_t).as_secs_f32() % 1.0 < 0.55 {
            let cx = x + 33.0 + shown as f32 * 12.0 - if n == 0 { 3.0 } else { 0.0 };
            g.gfx.Fill(cx, mid - 9.0, 1.5, 18.0, 0.5, GREEN.A(alpha));
        }
        self.Hit(Id::Field, r, true);
    }

    fn StatusLine(&mut self) -> (String, Rgba, bool) {
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
        let cur = self.paths.InstalledVersion();
        match (&self.remote, cur) {
            (Some(Ok(st)), _) if !st.updated => (format!("{} isn't updated yet", self.Gname()), WARN, false),
            (Some(Ok(st)), Some(v)) if st.version != v => (format!("Update available \u{b7} v{v} \u{2192} v{}", st.version), WARN, false),
            (_, Some(v)) if self.Installed() => (format!("{} v{v} \u{b7} up to date", self.Gname()), MUTED, false),
            (Some(Ok(st)), None) => (format!("Not installed \u{b7} v{} available", st.version), MUTED, false),
            (Some(Err(_)), None) => ("Couldn't reach linusware.org".into(), ERR, false),
            _ => ("Not installed".into(), MUTED, false),
        }
    }

    // LAUNCHER
    fn Shove(p: Panel) -> (f32, f32) {
        if matches!(p, Panel::Pick) { (0.0, 0.0) } else { (OX, OY) }
    }

    fn Park(g: &mut Gl, off: (f32, f32)) {
        g.gfx.ox = off.0;
        g.gfx.oy = off.1 - TOP;
    }

    pub(super) fn DrawLauncher(&mut self, g: &mut Gl, t: f32, motion: f32) {
        let it = (Instant::now() - self.intro).as_secs_f32();
        let k = self.panel_t;
        (self.hit_ox, self.hit_oy) = Self::Shove(self.panel);
        if k < 0.995 {
            Self::Park(g, Self::Shove(self.prev_panel));
            self.DrawPanel(g, self.prev_panel, 1.0 - k, false, t, motion, it);
        }
        Self::Park(g, Self::Shove(self.panel));
        self.DrawPanel(g, self.panel, k, k > 0.5, t, motion, it);
        Self::Park(g, (OX, OY));

        let main = matches!(self.panel, Panel::Main | Panel::Settings | Panel::Help | Panel::Changelog);
        if main {
            let gear = if self.panel == Panel::Settings { ICON_X } else { ICON_GEAR };
            let help = if matches!(self.panel, Panel::Help | Panel::Changelog) { ICON_X } else { ICON_HELP };
            let ia = (it * 3.0 - 1.2).clamp(0.0, 1.0);
            self.CircleButton(g, Id::Gear, 462.0, 70.0, gear, ia);
            self.CircleButton(g, Id::Help, 462.0, 344.0, help, ia);
        }
        if self.panel == Panel::Main && self.game.is_some() && self.panel_t > 0.3 {
            let a = ((it * 3.0 - 1.0).clamp(0.0, 1.0)) * self.panel_t;
            let r = [26.0, 54.0, 86.0, 28.0];
            self.Hit(Id::Back, r, true);
            let (hov, ..) = self.St(Id::Back);
            g.gfx.Rect(r[0], r[1], r[2], r[3], BoxStyle {
                fill: Some(LinusWare::gfx::Rgba::Hex(0x141e17).Mix(CIRCLE_HI, hov).A(0.7 * a)),
                radius: 14.0,
                ..Default::default()
            });
            g.gfx.Text(Font::Bold, 15.0, "\u{2039}", r[0] + 16.0, r[1] + 19.0, Align::Center, MINT.Mix(TITLE, hov).A(a));
            g.gfx.Text(Font::Medium, 12.5, "Games", r[0] + 28.0, r[1] + 18.5, Align::Left, MUTED.Mix(BODY, hov).A(a));
        }
        Self::Park(g, (0.0, 0.0));
    }

    fn DrawPick(&mut self, g: &mut Gl, alpha: f32, live: bool, it: f32) {
        let stage = |d: f32| ((it - d) / 0.45).clamp(0.0, 1.0);
        let pop = |x: f32| 1.0 - (1.0 - x).powi(3);

        let sh = pop(stage(0.0));
        g.gfx.Text(Font::Bold, 24.0, "Choose game", W * 0.5, 64.0 + (1.0 - sh) * 6.0, Align::Center, WHITE.A(alpha * sh));

        let (cw, ch, gap) = (212.0, 318.0, 36.0);
        let x0 = (W - cw * 2.0 - gap) * 0.5;
        for (n, game) in Game::ALL.into_iter().enumerate() {
            let st = pop(stage(0.1 + n as f32 * 0.08));
            let a = alpha * st;
            let id = game.Id();
            let (hov, press, ..) = self.St(id);
            let x = x0 + n as f32 * (cw + gap);
            let y = 84.0 + (1.0 - st) * 12.0 - 4.0 * hov + 2.0 * press;
            if live && st > 0.9 {
                self.Hit(id, [x, y, cw, ch], true);
            }
            g.gfx.Cover(game.Slot(), x, y, cw, ch, 14.0, a);
            g.gfx.Rect(x, y, cw, ch, BoxStyle {
                border: Some((LinusWare::gfx::Rgba::Hex(0xffffff).A((0.08 + 0.22 * hov) * a), 1.0)),
                radius: 14.0,
                ..Default::default()
            });
        }
    }
    fn DrawPanel(&mut self, g: &mut Gl, p: Panel, alpha: f32, live: bool, t: f32, motion: f32, it: f32) {
        if alpha <= 0.01 {
            return;
        }
        let dy = (1.0 - alpha) * 10.0;
        match p {
            Panel::Pick => self.DrawPick(g, alpha, live, it),
            Panel::Main => self.DrawMain(g, alpha, live, t, motion, it),
            Panel::Settings => self.DrawSettings(g, alpha, live, dy),
            Panel::Help => self.DrawHelp(g, alpha, live, dy),
            Panel::Changelog => self.DrawChangelog(g, alpha, live, dy),
        }
    }

    fn DrawMain(&mut self, g: &mut Gl, alpha: f32, live: bool, t: f32, motion: f32, it: f32) {
        let stage = |d: f32| ((it - d) / 0.45).clamp(0.0, 1.0);
        let pop = |x: f32| 1.0 - (1.0 - x).powi(3);
        let s0 = pop(stage(0.0));
        let float = (t * 1.6).sin() * 2.2 * motion;
        let breath = 0.5 + 0.5 * (t * 1.3).sin() * motion;

        let ls = 0.86 + 0.14 * s0;
        let (lx, ly, lw) = (250.0 - 50.0 * ls, 57.0 + float + (1.0 - s0) * 8.0, 100.0 * ls);
        let (pad, lpad) = g.glow_pad;
        g.gfx.Fx(200.0 - lpad, 57.0 + float - lpad, g.logo_glow, GLOW.A((0.08 + 0.05 * breath) * alpha * s0));
        g.gfx.Image(lx, ly, lw, lw, alpha * s0);

        let s1 = pop(stage(0.12));
        let tw = g.gfx.text.Measure(Font::Bold, 34.0, "LinusWare");
        let asc = g.gfx.text.Ascent(Font::Bold, 34.0);
        let ty = 194.0 + (1.0 - s1) * 8.0;
        g.gfx.Fx(250.0 - tw * 0.5 - pad, ty - asc - pad, g.title_glow, GLOW.A((0.26 + 0.1 * breath) * alpha * s1));
        g.gfx.Text(Font::Bold, 34.0, "LinusWare", 250.0, ty, Align::Center, TITLE.A(alpha * s1));

        let s2 = pop(stage(0.24));
        let (line, color, spin) = self.StatusLine();
        let line = g.gfx.text.Ellipsize(Font::Medium, 13.0, &line, 380.0);
        let lw2 = g.gfx.text.Measure(Font::Medium, 13.0, &line);
        let sa = alpha * s2 * (1.0 - self.ask_t * 0.0);
        let link = self.changelog && color == OK && line.starts_with("Updated to");
        if link {
            let head = format!("{line} \u{b7} ");
            let hw = g.gfx.text.Measure(Font::Medium, 13.0, &head);
            let lk = "See changelog";
            let kw = g.gfx.text.Measure(Font::Bold, 13.0, lk);
            let x0 = 250.0 - (hw + kw) * 0.5;
            g.gfx.Text(Font::Medium, 13.0, &head, x0, 221.0, Align::Left, color.A(sa));
            let r = [x0 + hw - 2.0, 207.0, kw + 4.0, 20.0];
            if live {
                self.Hit(Id::Changelog, r, true);
            }
            let (hov, ..) = self.St(Id::Changelog);
            g.gfx.Text(Font::Bold, 13.0, lk, x0 + hw, 221.0, Align::Left, TITLE.Mix(WHITE, hov).A(sa));
            g.gfx.Fill(x0 + hw, 223.5, kw * hov, 1.0, 0.5, TITLE.Mix(WHITE, hov).A(sa));
        } else if spin {
            g.gfx.Spinner(250.0 - lw2 * 0.5 - 12.0, 216.5, 5.5, 1.6, color.A(sa), t * 5.0);
            g.gfx.Text(Font::Medium, 13.0, &line, 250.0 + 7.0, 221.0, Align::Center, color.A(sa));
        } else {
            g.gfx.Text(Font::Medium, 13.0, &line, 250.0, 221.0, Align::Center, color.A(sa));
        }

        let ba = alpha * (1.0 - self.ask_t);
        let by = self.ask_t * 12.0;
        let s3 = pop(stage(0.34));
        let s4 = pop(stage(0.42));
        let installed = self.Installed();
        let idle = !self.Busy() && self.ask.is_none();
        let blive = live && self.ask_t < 0.5;
        let play = self.Gname();
        self.Button(g, Id::External, [109.0, 237.0 + by + (1.0 - s3) * 10.0, 132.0, 46.0], play, Btn::Secondary, installed && idle, ba * s3, blive);
        self.Button(g, Id::Delete, [259.0, 237.0 + by + (1.0 - s3) * 10.0, 132.0, 46.0], "Delete LinusWare", Btn::Danger, installed && (idle || self.job == Job::Idle), ba * s3, blive);

        let ur = [109.0, 301.0 + by + (1.0 - s4) * 10.0, 282.0, 46.0];
        if self.job == Job::Updating {
            let got = self.progress.got.load(Ordering::Relaxed) as f32;
            let total = self.progress.total.load(Ordering::Relaxed) as f32;
            let ph = self.progress.phase.load(Ordering::Relaxed);
            let frac = if ph == install::PH_DOWNLOAD && total > 0.0 { (got / total).min(1.0) } else if ph >= install::PH_VERIFY { 1.0 } else { 0.02 };
            let shown = self.Knob(Id::Update, frac);
            g.gfx.Rect(ur[0], ur[1], ur[2], ur[3], BoxStyle {
                fill: Some(LinusWare::gfx::Rgba::Hex(0x0b3d23).A(ba * s4)),
                radius: 7.0,
                progress: Some((shown.max(0.02), LinusWare::gfx::Rgba::Hex(0x00a352).A(ba * s4), t)),
                glow: Some((GLOW.A(0.1 * ba), 10.0)),
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
            g.gfx.Text(Font::Bold, 14.0, &label, 250.0, ur[1] + 28.0, Align::Center, WHITE.A(ba * s4));
            if blive {
                self.Hit(Id::Update, ur, false);
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
            self.Button(g, Id::Update, ur, label, Btn::Primary, enabled, ba * s4, blive);
            if let Some(e) = flash {
                let p = (e / 0.35).min(1.0);
                let w = g.gfx.text.Measure(Font::Bold, 14.0, label);
                g.gfx.Text(Font::Icon, 14.0 * (0.6 + 0.4 * p), "\u{f00c}", 250.0 - w * 0.5 - 13.0, ur[1] + 28.0, Align::Center, DARK_TEXT.A(ba * s4 * p));
            }
        }

        if self.ask_t > 0.01 {
            let a = alpha * self.ask_t;
            let oy = (1.0 - self.ask_t) * 12.0;
            let (label, c) = if self.ask_tries > 1 { ("Wrong password, try again", ERR) } else { ("Root password needed", BODY) };
            g.gfx.Text(Font::Medium, 13.0, &label, 250.0, 247.0 + oy, Align::Center, c.A(a));
            self.Field(g, [109.0, 256.0 + oy, 282.0, 40.0], a);
            let live2 = live && self.ask_t > 0.5;
            self.Button(g, Id::PwCancel, [109.0, 306.0 + oy, 132.0, 40.0], "Cancel", Btn::Secondary, true, a, live2);
            self.Button(g, Id::PwOk, [259.0, 306.0 + oy, 132.0, 40.0], "Unlock", Btn::Primary, !self.password.is_empty(), a, live2);
        }
    }

    fn Heading(&mut self, g: &mut Gl, s: &str, y: f32, alpha: f32) {
        g.gfx.Text(Font::Bold, 20.0, s, 40.0, y, Align::Left, TITLE.A(alpha));
    }

    fn DrawSettings(&mut self, g: &mut Gl, alpha: f32, live: bool, dy: f32) {
        LinusWare::app::draw::Scrim(g, alpha);
        self.Heading(g, "Settings", 78.0 + dy, alpha);
        let rows = [
            (Id::TogAuto, "Update on start", "Check for and install updates when the launcher opens", self.cfg.auto_update),
            (Id::TogClose, "Close after launch", "Hide the launcher once LinusWare is running", self.cfg.close_after_launch),
        ];
        for (i, (id, label, desc, val)) in rows.into_iter().enumerate() {
            let y = 98.0 + i as f32 * 54.0 + dy;
            let (hov, ..) = self.St(id);
            g.gfx.Fill(32.0, y, 436.0, 48.0, 8.0, LinusWare::gfx::Rgba::Hex(0xffffff).A(0.035 * hov * alpha));
            g.gfx.Text(Font::Bold, 14.0, label, 44.0, y + 21.0, Align::Left, BTN_ON_TEXT.A(alpha));
            g.gfx.Text(Font::Medium, 12.0, desc, 44.0, y + 38.0, Align::Left, SUBTLE.A(alpha));
            self.Toggle(g, id, 414.0, y + 12.0, val, alpha);
            if live {
                self.Hit(id, [32.0, y, 436.0, 48.0], true);
            }
        }
        let y = 212.0 + dy;
        g.gfx.Fill(40.0, y - 6.0, 420.0, 1.0, 0.0, LinusWare::gfx::Rgba::Hex(0xffffff).A(0.06 * alpha));
        g.gfx.Text(Font::Bold, 14.0, "Install folder", 44.0, y + 21.0, Align::Left, BTN_ON_TEXT.A(alpha));
        let path = self.paths.Pretty(&self.paths.install);
        let path = g.gfx.text.Ellipsize(Font::Medium, 12.0, &path, 300.0);
        g.gfx.Text(Font::Medium, 12.0, &path, 44.0, y + 38.0, Align::Left, SUBTLE.A(alpha));
        self.Button(g, Id::OpenFolder, [384.0, y + 9.0, 76.0, 32.0], "Open", Btn::Secondary, self.paths.install.is_dir(), alpha, live);
        g.gfx.Text(Font::Medium, 11.0, &format!("Launcher v{}", crate::LinusWare::VERSION), 40.0, 362.0 + dy, Align::Left, MUTED.A(alpha));
    }

    fn DrawHelp(&mut self, g: &mut Gl, alpha: f32, live: bool, dy: f32) {
        LinusWare::app::draw::Scrim(g, alpha);
        self.Heading(g, "Help", 78.0 + dy, alpha);
        let para = "Check for updates downloads the newest build from linusware.org and checks it before installing.";
        let lines = g.gfx.text.Wrap(Font::Medium, 15.0, para, 420.0);
        let mut y = 110.0 + dy;
        for l in &lines {
            g.gfx.Text(Font::Medium, 15.0, l, 40.0, y, Align::Left, BODY.A(alpha));
            y += 22.0;
        }
        y += 16.0;
        let ext = self.paths.InstalledVersion().map(|v| format!("v{v}")).unwrap_or_else(|| "not installed".into());
        let latest = match &self.remote {
            Some(Ok(s)) if s.updated => format!("  \u{b7}  latest v{}", s.version),
            _ => String::new(),
        };
        for (k, v) in [("Launcher", format!("v{}", crate::LinusWare::VERSION)), ("External", format!("{ext}{latest}"))] {
            g.gfx.Text(Font::Bold, 15.0, k, 40.0, y, Align::Left, BTN_ON_TEXT.A(alpha));
            g.gfx.Text(Font::Medium, 15.0, &v, 140.0, y, Align::Left, SUBTLE.A(alpha));
            y += 24.0;
        }
        let log = self.RemoteLog();
        if !log.is_empty() {
            y += 12.0;
            g.gfx.Text(Font::Bold, 15.0, "What's new", 40.0, y, Align::Left, BTN_ON_TEXT.A(alpha));
            let title = LinusWare::app::draw::LogTitle(&log);
            let title = g.gfx.text.Ellipsize(Font::Medium, 15.0, &title, 180.0);
            g.gfx.Text(Font::Medium, 15.0, &title, 140.0, y, Align::Left, SUBTLE.A(alpha));
            self.Button(g, Id::Changelog, [330.0, y - 21.0, 130.0, 30.0], "See changelog", Btn::Secondary, true, alpha, live);
        }
        let by = 304.0 + dy;
        self.Button(g, Id::Discord, [40.0, by, 120.0, 36.0], "Discord", Btn::Secondary, true, alpha, live);
        self.Button(g, Id::Backup, [170.0, by, 140.0, 36.0], "Backup Discord", Btn::Secondary, true, alpha, live);
        let has_log = LinusWare::launch::LogPath(&self.paths).exists();
        self.Button(g, Id::OpenLog, [320.0, by, 104.0, 36.0], "Open log", Btn::Secondary, has_log, alpha, live);
    }

    fn DrawChangelog(&mut self, g: &mut Gl, alpha: f32, live: bool, dy: f32) {
        LinusWare::app::draw::Scrim(g, alpha);
        self.Heading(g, "What's new", 78.0 + dy, alpha);
        if let Some(Ok(s)) = &self.remote {
            let hw = g.gfx.text.Measure(Font::Bold, 20.0, "What's new");
            g.gfx.Text(Font::Medium, 14.0, &format!("v{}", s.version), 40.0 + hw + 10.0, 78.0 + dy, Align::Left, SUBTLE.A(alpha));
        }
        let (vx, vy, vw, vh) = (40.0, 94.0 + dy, 400.0, 262.0);
        let log = self.RemoteLog();
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
                for (i, l) in g.gfx.text.Wrap(Font::Medium, 14.0, line, vw - 24.0).into_iter().enumerate() {
                    items.push((BODY, if i == 0 { dot } else { None }, Font::Medium, l));
                }
            }
        }
        let h: f32 = items.iter().map(|(_, _, f, t)| if t.is_empty() { 8.0 } else if *f == Font::Bold { 28.0 } else { 21.0 }).sum();
        self.log_max = (h - vh + 6.0).max(0.0);
        self.log_target = self.log_target.clamp(0.0, self.log_max);
        LinusWare::app::Ease(&mut self.log_scroll, self.log_target, self.dt, 16.0);
        g.gfx.clip = Some([vx - 4.0, vy, vw + 8.0, vh]);
        let mut y = vy + 16.0 - self.log_scroll;
        for (c, dot, f, t) in &items {
            if t.is_empty() {
                y += 8.0;
                continue;
            }
            if y > vy - 30.0 && y < vy + vh + 30.0 {
                if *f == Font::Bold {
                    g.gfx.Text(Font::Bold, 15.0, t, vx, y + 2.0, Align::Left, c.A(alpha));
                } else {
                    if let Some(d) = dot {
                        g.gfx.Fill(vx + 2.0, y - 8.0, 6.0, 6.0, 3.0, d.A(alpha));
                    }
                    g.gfx.Text(Font::Medium, 14.0, t, vx + 18.0, y, Align::Left, c.A(alpha));
                }
            }
            y += if *f == Font::Bold { 28.0 } else { 21.0 };
        }
        g.gfx.clip = None;
        if self.log_max > 0.0 {
            let th = (vh * vh / (vh + self.log_max)).max(24.0);
            let ty = vy + (vh - th) * (self.log_scroll / self.log_max).clamp(0.0, 1.0);
            g.gfx.Fill(vx + vw + 14.0, vy, 3.0, vh, 1.5, LinusWare::gfx::Rgba::Hex(0xffffff).A(0.05 * alpha));
            g.gfx.Fill(vx + vw + 14.0, ty, 3.0, th, 1.5, MINT.A(0.55 * alpha));
        }
        let _ = live;
    }

    // ASKPASS WINDOW
    pub(super) fn DrawAskpass(&mut self, g: &mut Gl, _t: f32) {
        let Mode::Askpass(prompt) = &self.mode else { return };
        let user = LinusWare::askpass::UserFromPrompt(prompt);
        let label = match user {
            Some(u) => format!("Password for {u}"),
            None => prompt.trim().trim_end_matches(':').to_string(),
        };
        g.gfx.Image(24.0, 44.0, 36.0, 36.0, 1.0);
        g.gfx.Text(Font::Bold, 19.0, "LinusWare", 70.0, 69.0, Align::Left, TITLE);
        let label = g.gfx.text.Ellipsize(Font::Medium, 13.0, &label, ASK_W - 48.0);
        g.gfx.Text(Font::Medium, 13.0, &label, 24.0, 106.0, Align::Left, BODY);
        self.Field(g, [24.0, 114.0, ASK_W - 48.0, 38.0], 1.0);
        self.Button(g, Id::PwCancel, [24.0, 162.0, 158.0, 38.0], "Cancel", Btn::Secondary, true, 1.0, true);
        self.Button(g, Id::PwOk, [198.0, 162.0, 158.0, 38.0], "Unlock", Btn::Primary, !self.password.is_empty(), 1.0, true);
    }

    fn Cursor(&self) -> CursorIcon {
        match self.hits.iter().rev().find(|h| LinusWare::app::Inside(h.r, self.mouse)) {
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
        if let Err(e) = self.Create(el) {
            self.err = Some(e);
            el.exit();
            return;
        }
        self.intro = Instant::now();
        if let Mode::Launcher = self.mode {
            if self.paths.ExeInstalled() {
                let _ = LinusWare::install::WriteDesktopEntry(&self.paths);
            }
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, msg: Msg) {
        self.OnMsg(msg);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => {
                self.CancelPassword();
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                self.Frame();
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
                    g.gfx.SetScale(scale_factor as f32);
                }
                self.BuildGlow();
            }
            WindowEvent::Focused(f) => self.focused = f,
            WindowEvent::Occluded(o) => self.hidden = o,
            WindowEvent::CursorMoved { position, .. } => {
                let s = self.gl.as_ref().map(|g| g.gfx.text.scale).unwrap_or(1.0);
                self.mouse = (position.x as f32 / s, position.y as f32 / s + TOP);
                self.mouse_in = true;
                if let Some(g) = &self.gl {
                    g.window.set_cursor(self.Cursor());
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.mouse_in = false;
                self.mouse = (-1e4, -1e4);
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => {
                    let hov = self.HoverId();
                    match hov {
                        Some(id) if self.Enabled(id) => {
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
                        if self.HoverId() == Some(p) {
                            self.Click(p);
                        }
                    }
                }
            },
            WindowEvent::KeyboardInput { event, .. } => self.Key(&event),
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

fn LogTitle(log: &str) -> String {
    let first = log.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    first.trim_matches('=').trim().to_string()
}
