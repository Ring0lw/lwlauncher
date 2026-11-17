use crate::LinusWare;
mod app;
mod askpass;
mod gfx;
mod install;
mod launch;
mod net;
mod remove;

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const EDITION: &str = "external";
pub const SOURCES: [&str; 2] = ["https://linusware.org/dl", "https://linusware.pages.dev/dl"];

// PATHS
#[derive(Clone)]
pub struct Paths {
    pub sources: Vec<String>,
    pub ed: String,
    pub install: PathBuf,
    pub exe: PathBuf,
    pub home: PathBuf,
}

impl Paths {
    pub fn New() -> Paths {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let install = match std::env::var_os("LINUSWARE_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => match std::env::var_os("XDG_DATA_HOME") {
                Some(d) if !d.is_empty() => PathBuf::from(d).join("linusware"),
                _ => home.join(".local/share/linusware"),
            },
        };
        let sources = match std::env::var("BASE_URL").ok().filter(|s| !s.trim().is_empty()) {
            Some(list) => list.split_whitespace().map(|s| s.trim_end_matches('/').to_string()).collect(),
            None => SOURCES.iter().map(|s| s.to_string()).collect(),
        };
        let exe = std::env::current_exe().unwrap_or_else(|_| install.join("lwlauncher"));
        Paths { sources, ed: EDITION.to_string(), install, exe, home }
    }

    pub fn Edition(&self) -> PathBuf {
        self.install.join(&self.ed)
    }

    pub fn ConfigDir(&self) -> PathBuf {
        match std::env::var_os("XDG_CONFIG_HOME") {
            Some(d) if !d.is_empty() => PathBuf::from(d).join("linusware"),
            _ => self.home.join(".config/linusware"),
        }
    }

    pub fn DataHome(&self) -> PathBuf {
        match std::env::var_os("XDG_DATA_HOME") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => self.home.join(".local/share"),
        }
    }

    pub fn DesktopFile(&self) -> PathBuf {
        self.DataHome().join("applications/linusware.desktop")
    }

    pub fn IconFile(&self) -> PathBuf {
        self.DataHome().join("icons/hicolor/scalable/apps/linusware.svg")
    }

    pub fn ExeInstalled(&self) -> bool {
        let dir = self.install.canonicalize().ok();
        let exe = self.exe.canonicalize().ok();
        matches!((&dir, &exe), (Some(d), Some(e)) if e.starts_with(d))
    }

    pub fn Installed(&self) -> bool {
        LinusWare::IsExec(&self.Edition().join("run.sh"))
    }

    pub fn InstalledVersion(&self) -> Option<String> {
        let v = std::fs::read_to_string(self.Edition().join("version.txt")).ok()?;
        let v = v.trim().to_string();
        (!v.is_empty()).then_some(v)
    }

    pub fn Pretty(&self, p: &Path) -> String {
        match p.strip_prefix(&self.home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => p.display().to_string(),
        }
    }
}

pub fn IsExec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

// CONFIG
#[derive(Clone, Copy)]
pub struct Config {
    pub auto_update: bool,
    pub close_after_launch: bool,
}

impl Config {
    pub fn Load(paths: &Paths) -> Config {
        let mut c = Config { auto_update: true, close_after_launch: true };
        if let Ok(s) = std::fs::read_to_string(paths.install.join("launcher.conf")) {
            for line in s.lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                let on = v.trim() == "1" || v.trim() == "true";
                match k.trim() {
                    "auto_update" => c.auto_update = on,
                    "close_after_launch" => c.close_after_launch = on,
                    _ => {}
                }
            }
        }
        c
    }

    pub fn Save(&self, paths: &Paths) {
        if !paths.install.is_dir() {
            return;
        }
        let b = |v: bool| if v { 1 } else { 0 };
        let s = format!(
            "auto_update={}\nclose_after_launch={}\n",
            b(self.auto_update),
            b(self.close_after_launch)
        );
        let _ = std::fs::write(paths.install.join("launcher.conf"), s);
    }
}

// CLI LOG
pub struct Log;

impl Log {
    pub fn Line(msg: &str) -> String {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = now.as_secs() as i64;
        let z = secs.div_euclid(86400) + 719468;
        let era = z.div_euclid(146097);
        let doe = z.rem_euclid(146097);
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        let rest = secs.rem_euclid(86400);
        format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z {msg}",
            rest / 3600,
            rest / 60 % 60,
            rest % 60,
            now.subsec_millis(),
        )
    }
    pub fn Info(msg: &str) {
        println!("{}", Self::Line(msg));
    }
    pub fn Warn(msg: &str) {
        println!("{}", Self::Line(msg));
    }
    pub fn Error(msg: &str) {
        println!("{}", Self::Line(msg));
    }
}

fn CliUpdate(paths: &Paths) -> bool {
    println!("------ LinusWare Updater ------\n");
    let mut ok = true;
    if paths.Installed() {
        match LinusWare::install::UpdateEdition(paths, &install::Progress::default(), false) {
            Ok(o) => LinusWare::Log::Info(&o.Describe()),
            Err(e) => {
                LinusWare::Log::Error(&format!("{EDITION}: {e}"));
                ok = false;
            }
        }
    } else {
        LinusWare::Log::Warn(&format!("{EDITION}: not installed, run with --install"));
    }
    LinusWare::install::RefreshScripts(paths, |m| LinusWare::Log::Info(&m));
    match LinusWare::install::SelfUpdate(paths, &install::Progress::default()) {
        Ok(Some(v)) => LinusWare::Log::Info(&format!("Launcher updated: v{v}")),
        Ok(None) => LinusWare::Log::Info("Launcher up to date"),
        Err(e) => LinusWare::Log::Warn(&format!("Launcher update failed: {e}")),
    }
    println!();
    LinusWare::Log::Info("Update finished");
    ok
}

fn CliInstall(paths: &Paths) -> bool {
    LinusWare::Log::Info(&format!("Installing LinusWare to {}", paths.install.display()));
    match LinusWare::install::UpdateEdition(paths, &install::Progress::default(), true) {
        Ok(o) => LinusWare::Log::Info(&o.Describe()),
        Err(e) => {
            LinusWare::Log::Error(&format!("{EDITION}: {e}"));
            return false;
        }
    }
    if paths.ExeInstalled() {
        let _ = LinusWare::install::WriteDesktopEntry(paths);
        LinusWare::Log::Info("Added LinusWare to your app menu");
    }
    LinusWare::Log::Info("Installation complete");
    true
}

fn CliUninstall(paths: &Paths) -> bool {
    let plan = match LinusWare::remove::MakePlan(paths) {
        Ok(p) => p,
        Err(e) => {
            LinusWare::Log::Error(&e);
            return false;
        }
    };
    println!("This removes:");
    for p in plan.All() {
        println!("  {}", paths.Pretty(&p));
    }
    if !LinusWare::AskYes("Delete LinusWare? [y/N] ") {
        return false;
    }
    let report = LinusWare::remove::Execute(&plan);
    for p in &report.failed {
        LinusWare::Log::Error(&format!("Failed to remove {} (try with sudo)", p.display()));
    }
    if report.failed.is_empty() {
        LinusWare::Log::Info("LinusWare removed");
    }
    let cfg = paths.ConfigDir();
    if cfg.exists() && LinusWare::AskYes(&format!("Also delete your configs and scripts in {}? [y/N] ", paths.Pretty(&cfg))) {
        let r = LinusWare::remove::RemoveConfig(paths);
        if r.failed.is_empty() {
            LinusWare::Log::Info("Configs removed");
        } else {
            LinusWare::Log::Error("Failed to remove some configs (try with sudo)");
        }
    }
    report.failed.is_empty()
}

fn AskYes(q: &str) -> bool {
    print!("{q}");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().lock().read_line(&mut s);
    matches!(s.trim(), "y" | "Y" | "yes" | "YES")
}

fn ExecEdition(paths: &Paths) -> ! {
    use std::os::unix::process::CommandExt;
    if !paths.Installed() {
        LinusWare::Log::Error("LinusWare is not installed. Run: lwlauncher --install");
        std::process::exit(1);
    }
    let err = std::process::Command::new("./run.sh").current_dir(paths.Edition()).exec();
    LinusWare::Log::Error(&format!("Failed to start run.sh: {err}"));
    std::process::exit(1);
}

fn TextMenu(paths: &Paths) -> i32 {
    loop {
        println!("\n\x1b[0;36m------ LinusWare Launcher ------\x1b[0m");
        if paths.Installed() {
            println!("  1) External (v{})", paths.InstalledVersion().unwrap_or_else(|| "?".into()));
        } else {
            println!("  i) Install LinusWare");
        }
        println!("  u) Check for Updates");
        println!("  d) Delete LinusWare");
        println!("  q) Quit\n");
        print!("Select: ");
        let _ = std::io::stdout().flush();
        let mut s = String::new();
        if std::io::stdin().lock().read_line(&mut s).unwrap_or(0) == 0 {
            return 0;
        }
        match s.trim() {
            "1" if paths.Installed() => LinusWare::ExecEdition(paths),
            "i" | "I" => {
                LinusWare::CliInstall(paths);
            }
            "u" | "U" => {
                LinusWare::CliUpdate(paths);
            }
            "d" | "D" => {
                LinusWare::CliUninstall(paths);
            }
            "q" | "Q" => return 0,
            _ => println!("\x1b[0;31mInvalid choice\x1b[0m"),
        }
    }
}

fn Usage() {
    println!(
        "LinusWare launcher v{VERSION}\n\n  lw              open the launcher\n  lw external     update, then start LinusWare in this terminal\n  lw --update     update LinusWare and the launcher\n  lw --install    install LinusWare\n  lw --uninstall  delete LinusWare\n\nLINUSWARE_DIR sets the install folder (default ~/.local/share/linusware)."
    );
}

// LEGACY
fn Migrate(paths: &Paths) {
    if std::env::var_os("LINUSWARE_DIR").is_some_and(|d| !d.is_empty()) {
        return;
    }
    let old = paths.home.join("linusware");
    let legacy = old.join("external/run.sh").exists() || (old.join("launcher.sh").exists() && old.join("update.sh").exists());
    if !legacy || old == paths.install {
        return;
    }
    if std::fs::create_dir_all(&paths.install).is_err() {
        return;
    }
    if !paths.Installed() && old.join("external").is_dir() && std::fs::rename(old.join("external"), paths.Edition()).is_err() {
        return;
    }
    if !paths.install.join("launcher.conf").exists() {
        let _ = std::fs::rename(old.join("launcher.conf"), paths.install.join("launcher.conf"));
    }
    for e in ["external", "internal", "launcher.sh", "update.sh", ".scripts", "lwlauncher", "launcher.conf", ".lwlauncher.new"] {
        let p = old.join(e);
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => {
                let _ = std::fs::remove_dir_all(&p);
            }
            Ok(_) => {
                let _ = std::fs::remove_file(&p);
            }
            Err(_) => {}
        }
    }
    let _ = std::fs::remove_dir(&old);
}

pub fn Main() {
    let paths = LinusWare::Paths::New();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str).unwrap_or("");
    if std::env::var_os("LW_ASKPASS_TOKEN").is_some() && !first.starts_with("--") || first.to_ascii_lowercase().contains("password") {
        std::process::exit(LinusWare::askpass::ClientMain(if first.is_empty() { "Password: " } else { first }));
    }
    LinusWare::Migrate(&paths);
    if paths.ExeInstalled() {
        let _ = LinusWare::install::WriteDesktopEntry(&paths);
    }
    let code = match args.first().map(String::as_str) {
        Some("--askpass") => LinusWare::askpass::ClientMain(args.get(1).map(String::as_str).unwrap_or("Password: ")),
        Some("--update") => (!LinusWare::CliUpdate(&paths)) as i32,
        Some("--install") => (!LinusWare::CliInstall(&paths)) as i32,
        Some("--uninstall") => (!LinusWare::CliUninstall(&paths)) as i32,
        Some("--version" | "-V") => {
            println!("lwlauncher {VERSION}");
            0
        }
        Some("--help" | "-h") => {
            LinusWare::Usage();
            0
        }
        Some(ed) if ed.eq_ignore_ascii_case(EDITION) => {
            LinusWare::CliUpdate(&paths);
            LinusWare::ExecEdition(&paths)
        }
        Some(other) => {
            LinusWare::Log::Error(&format!("Unknown edition: {other}"));
            1
        }
        None => match LinusWare::app::Run(paths.clone(), app::Mode::Launcher) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("{}", LinusWare::Log::Line(&format!("No window ({e}), using text menu")));
                if std::io::stdin().is_terminal() { LinusWare::TextMenu(&paths) } else { 1 }
            }
        },
    };
    std::process::exit(code);
}
