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
pub const DEFAULT_BASE: &str = "https://git.catapis.uk/linusware/main/raw/branch/main";

// PATHS
#[derive(Clone)]
pub struct Paths {
    pub base_url: String,
    pub install: PathBuf,
    pub exe: PathBuf,
    pub home: PathBuf,
}

impl Paths {
    pub fn new() -> Paths {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let install = match std::env::var_os("LINUSWARE_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => match std::env::var_os("XDG_DATA_HOME") {
                Some(d) if !d.is_empty() => PathBuf::from(d).join("linusware"),
                _ => home.join(".local/share/linusware"),
            },
        };
        let base_url = std::env::var("BASE_URL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| DEFAULT_BASE.into());
        let exe = std::env::current_exe().unwrap_or_else(|_| install.join("lwlauncher"));
        Paths { base_url: base_url.trim_end_matches('/').into(), install, exe, home }
    }

    pub fn edition(&self) -> PathBuf {
        self.install.join(EDITION)
    }

    pub fn url(&self, rel: &str) -> String {
        format!("{}/{}", self.base_url, rel)
    }

    pub fn config_dir(&self) -> PathBuf {
        match std::env::var_os("XDG_CONFIG_HOME") {
            Some(d) if !d.is_empty() => PathBuf::from(d).join("linusware"),
            _ => self.home.join(".config/linusware"),
        }
    }

    pub fn data_home(&self) -> PathBuf {
        match std::env::var_os("XDG_DATA_HOME") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => self.home.join(".local/share"),
        }
    }

    pub fn desktop_file(&self) -> PathBuf {
        self.data_home().join("applications/linusware.desktop")
    }

    pub fn icon_file(&self) -> PathBuf {
        self.data_home().join("icons/hicolor/scalable/apps/linusware.svg")
    }

    pub fn exe_installed(&self) -> bool {
        let dir = self.install.canonicalize().ok();
        let exe = self.exe.canonicalize().ok();
        matches!((&dir, &exe), (Some(d), Some(e)) if e.starts_with(d))
    }

    pub fn installed(&self) -> bool {
        is_exec(&self.edition().join("run.sh"))
    }

    pub fn installed_version(&self) -> Option<String> {
        let v = std::fs::read_to_string(self.edition().join("version.txt")).ok()?;
        let v = v.trim().to_string();
        (!v.is_empty()).then_some(v)
    }

    pub fn pretty(&self, p: &Path) -> String {
        match p.strip_prefix(&self.home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => p.display().to_string(),
        }
    }
}

pub fn is_exec(p: &Path) -> bool {
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
    pub fn load(paths: &Paths) -> Config {
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

    pub fn save(&self, paths: &Paths) {
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
    fn tag(color: &str, tag: &str, msg: &str) {
        if std::io::stdout().is_terminal() {
            println!("\x1b[{color}m[{tag}]\x1b[0m {msg}");
        } else {
            println!("[{tag}] {msg}");
        }
    }
    pub fn info(msg: &str) {
        Self::tag("0;32", "*", msg)
    }
    pub fn warn(msg: &str) {
        Self::tag("1;33", "!", msg)
    }
    pub fn error(msg: &str) {
        Self::tag("0;31", "x", msg)
    }
}

fn cli_update(paths: &Paths) -> bool {
    println!("------ LinusWare Updater ------\n");
    let mut ok = true;
    if paths.installed() {
        match install::update_edition(paths, &install::Progress::default(), false) {
            Ok(o) => Log::info(&o.describe()),
            Err(e) => {
                Log::error(&format!("{EDITION}: {e}"));
                ok = false;
            }
        }
    } else {
        Log::warn(&format!("{EDITION}: not installed, run with --install"));
    }
    install::refresh_scripts(paths, |m| Log::info(&m));
    match install::self_update(paths, &install::Progress::default()) {
        Ok(Some(v)) => Log::info(&format!("launcher: updated to v{v}")),
        Ok(None) => Log::info("launcher: up-to-date"),
        Err(e) => Log::warn(&format!("launcher: {e}")),
    }
    println!();
    Log::info("Done.");
    ok
}

fn cli_install(paths: &Paths) -> bool {
    Log::info(&format!("Installing LinusWare to {}", paths.install.display()));
    match install::update_edition(paths, &install::Progress::default(), true) {
        Ok(o) => Log::info(&o.describe()),
        Err(e) => {
            Log::error(&format!("{EDITION}: {e}"));
            return false;
        }
    }
    if paths.exe_installed() {
        let _ = install::write_desktop_entry(paths);
        Log::info("Added LinusWare to your app menu");
    }
    Log::info("Installation complete!");
    true
}

fn cli_uninstall(paths: &Paths) -> bool {
    let plan = match remove::plan(paths) {
        Ok(p) => p,
        Err(e) => {
            Log::error(&e);
            return false;
        }
    };
    println!("This removes:");
    for p in plan.all() {
        println!("  {}", paths.pretty(&p));
    }
    if !ask_yes("Delete LinusWare? [y/N] ") {
        return false;
    }
    let report = remove::execute(&plan);
    for p in &report.failed {
        Log::error(&format!("could not remove {} (try with sudo)", p.display()));
    }
    if report.failed.is_empty() {
        Log::info("LinusWare removed");
    }
    let cfg = paths.config_dir();
    if cfg.exists() && ask_yes(&format!("Also delete your configs and scripts in {}? [y/N] ", paths.pretty(&cfg))) {
        let r = remove::remove_config(paths);
        if r.failed.is_empty() {
            Log::info("Configs removed");
        } else {
            Log::error("some configs could not be removed (try with sudo)");
        }
    }
    report.failed.is_empty()
}

fn ask_yes(q: &str) -> bool {
    print!("{q}");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().lock().read_line(&mut s);
    matches!(s.trim(), "y" | "Y" | "yes" | "YES")
}

fn exec_edition(paths: &Paths) -> ! {
    use std::os::unix::process::CommandExt;
    if !paths.installed() {
        Log::error("LinusWare is not installed. Run: lwlauncher --install");
        std::process::exit(1);
    }
    let err = std::process::Command::new("./run.sh").current_dir(paths.edition()).exec();
    Log::error(&format!("could not start run.sh: {err}"));
    std::process::exit(1);
}

fn text_menu(paths: &Paths) -> i32 {
    loop {
        println!("\n\x1b[0;36m------ LinusWare Launcher ------\x1b[0m");
        if paths.installed() {
            println!("  1) External (v{})", paths.installed_version().unwrap_or_else(|| "?".into()));
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
            "1" if paths.installed() => exec_edition(paths),
            "i" | "I" => {
                cli_install(paths);
            }
            "u" | "U" => {
                cli_update(paths);
            }
            "d" | "D" => {
                cli_uninstall(paths);
            }
            "q" | "Q" => return 0,
            _ => println!("\x1b[0;31mInvalid choice\x1b[0m"),
        }
    }
}

fn usage() {
    println!(
        "LinusWare launcher v{VERSION}\n\n  lwlauncher              open the launcher\n  lwlauncher external     update, then start LinusWare in this terminal\n  lwlauncher --update     update LinusWare and the launcher\n  lwlauncher --install    install LinusWare\n  lwlauncher --uninstall  delete LinusWare\n\nLINUSWARE_DIR sets the install folder (default ~/.local/share/linusware)."
    );
}

// LEGACY
fn migrate(paths: &Paths) {
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
    if !paths.installed() && old.join("external").is_dir() && std::fs::rename(old.join("external"), paths.edition()).is_err() {
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

fn main() {
    let paths = Paths::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str).unwrap_or("");
    if std::env::var_os("LW_ASKPASS_TOKEN").is_some() && !first.starts_with("--") || first.to_ascii_lowercase().contains("password") {
        std::process::exit(askpass::client_main(if first.is_empty() { "Password: " } else { first }));
    }
    migrate(&paths);
    let code = match args.first().map(String::as_str) {
        Some("--askpass") => askpass::client_main(args.get(1).map(String::as_str).unwrap_or("Password: ")),
        Some("--update") => (!cli_update(&paths)) as i32,
        Some("--install") => (!cli_install(&paths)) as i32,
        Some("--uninstall") => (!cli_uninstall(&paths)) as i32,
        Some("--version" | "-V") => {
            println!("lwlauncher {VERSION}");
            0
        }
        Some("--help" | "-h") => {
            usage();
            0
        }
        Some(ed) if ed.eq_ignore_ascii_case(EDITION) => {
            cli_update(&paths);
            exec_edition(&paths)
        }
        Some(other) => {
            Log::error(&format!("Unknown edition: {other}"));
            1
        }
        None => match app::run(paths.clone(), app::Mode::Launcher) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("lwlauncher: no window ({e}), using the text menu");
                if std::io::stdin().is_terminal() { text_menu(&paths) } else { 1 }
            }
        },
    };
    std::process::exit(code);
}
