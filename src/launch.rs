use crate::Paths;
use crate::askpass::Bridge;
use std::collections::HashMap;
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

fn sudo(paths: &Paths, bridge: &Bridge, dir: &Path, args: &[&str], log: &Path) -> std::io::Result<Child> {
    let out = std::fs::File::create(log)?;
    let err = out.try_clone()?;
    #[cfg(debug_assertions)]
    let bin = std::env::var("LW_SUDO").unwrap_or_else(|_| "sudo".into());
    #[cfg(not(debug_assertions))]
    let bin = "sudo";
    let mut c = Command::new(bin);
    c.arg("-A").args(args).current_dir(dir);
    c.env("SUDO_ASKPASS", &paths.exe).env("LW_ASKPASS", &bridge.path).env("LW_ASKPASS_TOKEN", &bridge.token);
    c.stdin(Stdio::null()).stdout(out).stderr(err);
    unsafe {
        c.pre_exec(|| {
            setsid();
            Ok(())
        });
    }
    c.spawn()
}

// EXTERNAL
pub fn start(paths: &Paths, bridge: &Bridge, mut send: impl FnMut(Event) + Send + 'static) -> std::io::Result<()> {
    let log = log_path(paths);
    let mut child = sudo(paths, bridge, &paths.edition(), &["-E", "./run.sh"], &log)?;
    let paths = paths.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut seen: Option<Instant> = None;
        loop {
            if let Ok(Some(st)) = child.try_wait() {
                let code = st.code().unwrap_or(-1);
                let t = tail(&log);
                if seen.is_none() && code != 0 && unsupported(&t) && started.elapsed() < Duration::from_secs(3) {
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
    let mut args: Vec<String> = vec!["rm".into(), "-rf".into(), "--".into()];
    args.extend(targets.iter().map(|p| p.display().to_string()));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut child = sudo(paths, bridge, Path::new("/"), &refs, &log)?;
    std::thread::spawn(move || {
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        let t = tail(&log);
        let _ = std::fs::remove_file(&log);
        done(ok, t);
    });
    Ok(())
}

pub fn open(target: &str) {
    let _ = Command::new("xdg-open").arg(target).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
}
