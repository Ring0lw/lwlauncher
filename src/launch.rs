use crate::Paths;
use crate::askpass::Bridge;
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub enum Event {
    Running,
    Stable,
    Exited { code: i32, tail: String },
}

unsafe extern "C" {
    fn setsid() -> i32;
}

// PROC
fn procs() -> Vec<(u32, u32, String)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return out };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else { continue };
        let (Some(l), Some(r)) = (stat.find('('), stat.rfind(')')) else { continue };
        let comm = stat[l + 1..r].to_string();
        let ppid = stat[r + 1..].split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        out.push((pid, ppid, comm));
    }
    out
}

pub fn running(paths: &Paths) -> bool {
    let dir = paths.install.canonicalize().unwrap_or_else(|_| paths.install.clone());
    procs().iter().filter(|(_, _, c)| c == "linusware").any(|(pid, _, _)| match std::fs::read_link(format!("/proc/{pid}/exe")) {
        Ok(exe) => exe.starts_with(&dir),
        Err(_) => true,
    })
}

fn descendant_named(root: u32, name: &str) -> bool {
    let all = procs();
    let parent: HashMap<u32, u32> = all.iter().map(|(p, pp, _)| (*p, *pp)).collect();
    all.iter().filter(|(_, _, c)| c == name).any(|(pid, _, _)| {
        if *pid == root {
            return true;
        }
        let mut cur = *pid;
        for _ in 0..16 {
            match parent.get(&cur) {
                Some(&p) if p == root => return true,
                Some(&p) if p > 1 => cur = p,
                _ => return false,
            }
        }
        false
    })
}

pub fn log_path(paths: &Paths) -> PathBuf {
    paths.edition().join("last.log")
}

pub fn tail(p: &Path) -> String {
    let s = std::fs::read_to_string(p).unwrap_or_default();
    s.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(160).collect()
}

unsafe extern "C" {
    fn getuid() -> u32;
    fn getgid() -> u32;
}

const KEEP_ENV: [&str; 14] = [
    "HOME", "USER", "LOGNAME", "PATH", "LANG", "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "XDG_RUNTIME_DIR",
    "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "DBUS_SESSION_BUS_ADDRESS",
];

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join(bin))
        .find(|p| crate::is_exec(p))
}

fn pkexec_bin() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("LW_PKEXEC") {
        return (!p.is_empty()).then(|| PathBuf::from(p));
    }
    which("pkexec")
}

#[derive(Clone)]
struct Ask {
    exe: PathBuf,
    sock: PathBuf,
    token: String,
}

impl Ask {
    fn new(paths: &Paths, bridge: &Bridge) -> Ask {
        Ask { exe: paths.exe.clone(), sock: bridge.path.clone(), token: bridge.token.clone() }
    }
}

// ELEVATE
fn elevate(pkexec: Option<&Path>, ask: &Ask, dir: &Path, cmd: &[OsString], log: &Path) -> std::io::Result<Child> {
    let out = std::fs::File::create(log)?;
    let err = out.try_clone()?;
    let mut c = match pkexec {
        Some(pk) => {
            let mut c = Command::new(pk);
            c.arg("--disable-internal-agent").arg(which("env").unwrap_or_else(|| PathBuf::from("/usr/bin/env")));
            let vars = KEEP_ENV.iter().map(|k| (OsString::from(k), std::env::var_os(k))).chain(
                std::env::vars_os().filter(|(k, _)| k.to_str().is_some_and(|k| k.starts_with("LW_") && !k.starts_with("LW_ASKPASS"))).map(|(k, v)| (k, Some(v))),
            );
            for (k, v) in vars {
                if let Some(v) = v {
                    let mut kv = k;
                    kv.push("=");
                    kv.push(v);
                    c.arg(kv);
                }
            }
            let (uid, gid) = unsafe { (getuid(), getgid()) };
            c.arg(format!("SUDO_UID={uid}")).arg(format!("SUDO_GID={gid}"));
            if let Some(u) = std::env::var_os("USER") {
                let mut kv = OsString::from("SUDO_USER=");
                kv.push(u);
                c.arg(kv);
            }
            c
        }
        None => {
            #[cfg(debug_assertions)]
            let bin = std::env::var("LW_SUDO").unwrap_or_else(|_| "sudo".into());
            #[cfg(not(debug_assertions))]
            let bin = "sudo";
            let mut c = Command::new(bin);
            c.args(["-A", "-E"]);
            c.env("SUDO_ASKPASS", &ask.exe).env("LW_ASKPASS", &ask.sock).env("LW_ASKPASS_TOKEN", &ask.token);
            c
        }
    };
    c.args(cmd).current_dir(dir);
    c.stdin(Stdio::null()).stdout(out).stderr(err);
    unsafe {
        c.pre_exec(|| {
            setsid();
            Ok(())
        });
    }
    c.spawn()
}

fn no_agent(code: i32, tail: &str) -> bool {
    let t = tail.to_ascii_lowercase();
    code == 127 && (t.contains("authentication agent") || t.contains("no session for cookie"))
}

pub const CANCELLED: i32 = 126;

// EXTERNAL
pub fn start(paths: &Paths, bridge: &Bridge, mut send: impl FnMut(Event) + Send + 'static) -> std::io::Result<()> {
    let log = log_path(paths);
    let dir = paths.edition();
    let cmd = [dir.join("run.sh").into_os_string()];
    let ask = Ask::new(paths, bridge);
    let mut pk = pkexec_bin();
    let mut child = elevate(pk.as_deref(), &ask, &dir, &cmd, &log)?;
    let paths = paths.clone();
    std::thread::spawn(move || {
        let mut started = Instant::now();
        let mut seen: Option<Instant> = None;
        loop {
            if let Ok(Some(st)) = child.try_wait() {
                let code = st.code().unwrap_or(-1);
                let t = tail(&log);
                if pk.is_some() && seen.is_none() && no_agent(code, &t) {
                    pk = None;
                    match elevate(None, &ask, &dir, &cmd, &log) {
                        Ok(c) => {
                            child = c;
                            started = Instant::now();
                            continue;
                        }
                        Err(e) => {
                            send(Event::Exited { code, tail: e.to_string() });
                            return;
                        }
                    }
                }
                if pk.is_some() && seen.is_none() && code == CANCELLED {
                    send(Event::Exited { code, tail: String::new() });
                    return;
                }
                if pk.is_none() && seen.is_none() && code != 0 && unsupported(&t) && started.elapsed() < Duration::from_secs(3) {
                    match in_terminal(&paths) {
                        Ok(()) => send(Event::Stable),
                        Err(_) => send(Event::Exited { code, tail: t }),
                    }
                    return;
                }
                send(Event::Exited { code, tail: t });
                return;
            }
            match seen {
                None if descendant_named(child.id(), "linusware") => {
                    seen = Some(Instant::now());
                    send(Event::Running);
                }
                Some(t) if t.elapsed() > Duration::from_millis(1500) => {
                    send(Event::Stable);
                    seen = Some(Instant::now() + Duration::from_secs(3600 * 24));
                }
                _ => {}
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    Ok(())
}

fn unsupported(tail: &str) -> bool {
    let t = tail.to_ascii_lowercase();
    ["unexpected argument", "invalid option", "unrecognized option", "askpass"].iter().any(|k| t.contains(k)) && !t.contains("incorrect")
}

fn in_terminal(paths: &Paths) -> std::io::Result<()> {
    let dir = paths.edition();
    let cmd = format!("cd '{}' && ./run.sh; echo; read -rp 'Press Enter to close' _", dir.display().to_string().replace('\'', "'\\''"));
    let mut terms: Vec<String> = Vec::new();
    if let Ok(t) = std::env::var("TERMINAL") {
        terms.push(t);
    }
    for t in ["x-terminal-emulator", "kitty", "alacritty", "foot", "wezterm", "ghostty", "konsole", "gnome-terminal", "xfce4-terminal", "xterm"] {
        terms.push(t.into());
    }
    for t in terms {
        let name = Path::new(&t).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let mut c = Command::new(&t);
        match name.as_str() {
            "gnome-terminal" => c.args(["--", "sh", "-c", &cmd]),
            "wezterm" => c.args(["start", "--", "sh", "-c", &cmd]),
            "kitty" | "foot" => c.args(["sh", "-c", &cmd]),
            "xfce4-terminal" => c.args(["-x", "sh", "-c", &cmd]),
            _ => c.args(["-e", "sh", "-c", &cmd]),
        };
        c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        unsafe {
            c.pre_exec(|| {
                setsid();
                Ok(())
            });
        }
        if c.spawn().is_ok() {
            return Ok(());
        }
    }
    Err(std::io::Error::other("no terminal found"))
}

// ELEVATED DELETE
pub fn sudo_rm(paths: &Paths, bridge: &Bridge, targets: &[PathBuf], mut done: impl FnMut(bool, String) + Send + 'static) -> std::io::Result<()> {
    let log = std::env::temp_dir().join(format!("lwlauncher-rm-{}.log", std::process::id()));
    let mut cmd: Vec<OsString> = vec![which("rm").unwrap_or_else(|| PathBuf::from("/usr/bin/rm")).into_os_string(), "-rf".into(), "--".into()];
    cmd.extend(targets.iter().map(|p| p.clone().into_os_string()));
    let ask = Ask::new(paths, bridge);
    let pk = pkexec_bin();
    let mut child = elevate(pk.as_deref(), &ask, Path::new("/"), &cmd, &log)?;
    std::thread::spawn(move || {
        let mut st = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        if pk.is_some() && no_agent(st, &tail(&log)) {
            st = elevate(None, &ask, Path::new("/"), &cmd, &log).and_then(|mut c| c.wait()).map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        }
        let t = if st == CANCELLED { String::new() } else { tail(&log) };
        let _ = std::fs::remove_file(&log);
        done(st == 0, t);
    });
    Ok(())
}

pub fn open(target: &str) {
    let _ = Command::new("xdg-open").arg(target).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
}
