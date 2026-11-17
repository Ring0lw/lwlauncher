use crate::LinusWare;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// STATUS
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub version: String,
    pub updated: bool,
    pub log: String,
}

impl Status {
    pub fn Parse(s: &str) -> Option<Status> {
        let version = LinusWare::net::JsonStr(s, "version")?;
        let updated = LinusWare::net::JsonBool(s, "updated").unwrap_or(false);
        let log = LinusWare::net::JsonStr(s, "log").unwrap_or_default();
        Some(Status { version, updated, log })
    }
}

fn JsonValue<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\"");
    let i = s.find(&pat)? + pat.len();
    let rest = s[i..].trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim_start())
}

fn JsonBool(s: &str, key: &str) -> Option<bool> {
    let v = LinusWare::net::JsonValue(s, key)?;
    if v.starts_with("true") {
        Some(true)
    } else if v.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn JsonStr(s: &str, key: &str) -> Option<String> {
    let v = LinusWare::net::JsonValue(s, key)?.strip_prefix('"')?;
    let mut out = String::new();
    let mut it = v.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = it.by_ref().take(4).collect();
                    out.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).unwrap_or('?'));
                }
                o => out.push(o),
            },
            c => out.push(c),
        }
    }
    None
}

// CURL
pub fn HaveCurl() -> bool {
    Command::new("curl").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn Curl() -> Command {
    let mut c = Command::new("curl");
    c.args(["-fsSL", "--proto-redir", "=https", "--connect-timeout", "10", "-A", concat!("lwlauncher/", env!("CARGO_PKG_VERSION"))]);
    c.stdin(Stdio::null());
    c
}

fn CurlErr(out: &std::process::Output) -> String {
    let e = String::from_utf8_lossy(&out.stderr);
    let e = e.trim().trim_start_matches("curl: ");
    if e.is_empty() { format!("download failed ({})", out.status) } else { e.to_string() }
}

pub fn Fetch(url: &str) -> Result<String, String> {
    let out = LinusWare::net::Curl().args(["--max-time", "20", url]).output().map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(LinusWare::net::CurlErr(&out));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn Head(url: &str) -> Result<(u64, String), String> {
    let out = LinusWare::net::Curl().args(["--max-time", "20", "-I", url]).output().map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(LinusWare::net::CurlErr(&out));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut len = 0;
    let mut etag = String::new();
    let mut modified = String::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = v.trim().to_string();
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => len = v.parse().unwrap_or(len),
            "etag" => etag = v,
            "last-modified" => modified = v,
            _ => {}
        }
    }
    Ok((len, if etag.is_empty() { modified } else { etag }))
}

pub fn Download(url: &str, dest: &Path, got: &AtomicU64) -> Result<(), String> {
    let mut child = LinusWare::net::Curl()
        .args(["--max-time", "600", "-o"])
        .arg(dest)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;
    loop {
        if let Ok(m) = std::fs::metadata(dest) {
            got.store(m.len(), Ordering::Relaxed);
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(e) => return Err(e.to_string()),
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if let Ok(m) = std::fs::metadata(dest) {
        got.store(m.len(), Ordering::Relaxed);
    }
    if !out.status.success() {
        let _ = std::fs::remove_file(dest);
        return Err(LinusWare::net::CurlErr(&out));
    }
    Ok(())
}

pub fn Newer(remote: &str, local: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> { s.trim().trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (parse(remote), parse(local));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}
