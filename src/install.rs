use crate::net::{self, Status};
use crate::{EDITION, Paths};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

pub const ICON_SVG: &str = include_str!("../assets/linusware.svg");

// PROGRESS
#[derive(Default)]
pub struct Progress {
    pub got: AtomicU64,
    pub total: AtomicU64,
    pub phase: AtomicU8,
}

pub const PH_IDLE: u8 = 0;
pub const PH_DOWNLOAD: u8 = 1;
pub const PH_VERIFY: u8 = 2;
pub const PH_INSTALL: u8 = 3;
pub const PH_LAUNCHER: u8 = 4;

impl Progress {
    pub fn set(&self, p: u8) {
        self.phase.store(p, Ordering::Relaxed);
    }
}

pub enum Outcome {
    UpToDate(String, String),
    Updated { from: Option<String>, to: String, log: String },
    NotReleased,
}

impl Outcome {
    pub fn describe(&self) -> String {
        match self {
            Outcome::UpToDate(v, _) => format!("{EDITION}: already up-to-date (v{v})"),
            Outcome::Updated { from, to, .. } => format!("{EDITION}: {} -> v{to}", from.as_deref().unwrap_or("none")),
            Outcome::NotReleased => format!("{EDITION}: not available (not yet released)"),
        }
    }
}

pub fn status(paths: &Paths, what: &str) -> Result<Status, String> {
    let text = net::fetch(&paths.url(&format!("{what}/status.json")))?;
    Status::parse(&text).ok_or_else(|| "bad status.json".into())
}

pub const RELEASES: &str = "https://github.com/Ring0lw/lwlauncher/releases";
pub const LATEST_API: &str = "https://api.github.com/repos/Ring0lw/lwlauncher/releases/latest";

fn md5sum(url: &str) -> Result<String, String> {
    let text = net::fetch(url)?;
    let hex: String = text.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("bad MD5SUM".into());
    }
    Ok(hex)
}

fn fetch_verified(url: &str, md5_url: &str, dest: &Path, pr: &Progress) -> Result<(), String> {
    let expected = md5sum(md5_url)?;
    let total = net::head(url).map(|(n, _)| n).unwrap_or(0);
    pr.total.store(total, Ordering::Relaxed);
    pr.got.store(0, Ordering::Relaxed);
    pr.set(PH_DOWNLOAD);
    net::download(url, dest, &pr.got)?;
    pr.set(PH_VERIFY);
    let actual = net::md5_file(dest).map_err(|e| e.to_string())?;
    if actual != expected {
        let _ = std::fs::remove_file(dest);
        return Err(format!("MD5 mismatch (expected {expected}, got {actual})"));
    }
    Ok(())
}

// EDITION
pub fn update_edition(paths: &Paths, pr: &Progress, force: bool) -> Result<Outcome, String> {
    let st = status(paths, EDITION)?;
    if !st.updated {
        return Ok(Outcome::NotReleased);
    }
    let dir = paths.edition();
    let cur = paths.installed_version();
    if !force && paths.installed() && cur.as_deref() == Some(st.version.as_str()) {
        return Ok(Outcome::UpToDate(st.version, st.log));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let archive = dir.join(".download.tmp");
    let result = (|| {
        fetch_verified(&paths.url(&format!("{EDITION}/bin/linusware.tar.gz")), &paths.url(&format!("{EDITION}/bin/MD5SUM")), &archive, pr)?;
        pr.set(PH_INSTALL);
        unpack(&archive, &dir)?;
        write_atomic(&dir.join("version.txt"), format!("{}\n", st.version).as_bytes()).map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(&archive);
    pr.set(PH_IDLE);
    result?;
    Ok(Outcome::Updated { from: cur, to: st.version, log: st.log })
}

fn unpack(archive: &Path, dir: &Path) -> Result<(), String> {
    let staging = dir.join(".staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(f));
    ar.set_preserve_permissions(false);
    ar.set_overwrite(true);
    if let Err(e) = ar.unpack(&staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("unpack: {e}"));
    }
    if !staging.join("run.sh").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("run.sh not found after extraction".into());
    }
    let entries: Vec<_> = std::fs::read_dir(&staging).map_err(|e| e.to_string())?.flatten().collect();
    for e in entries {
        let name = e.file_name();
        let target = dir.join(&name);
        if let Ok(m) = std::fs::symlink_metadata(&target) {
            if m.is_dir() {
                let _ = std::fs::remove_dir_all(&target);
            } else {
                let _ = std::fs::remove_file(&target);
            }
        }
        std::fs::rename(e.path(), &target).map_err(|err| format!("{}: {err}", target.display()))?;
    }
    let _ = std::fs::remove_dir_all(&staging);
    let run = dir.join("run.sh");
    let _ = std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o755));
    Ok(())
}

fn write_atomic(p: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, p)
}

// SELF UPDATE
pub fn self_update(paths: &Paths, pr: &Progress) -> Result<Option<String>, String> {
    if !paths.exe_installed() {
        return Ok(None);
    }
    let Ok(tag) = net::latest_tag(LATEST_API, RELEASES) else { return Ok(None) };
    let version = tag.trim_start_matches('v').to_string();
    if !net::newer(&version, crate::VERSION) {
        return Ok(None);
    }
    let tmp = paths.install.join(".lwlauncher.new");
    let base = format!("{RELEASES}/download/{tag}");
    let r = fetch_verified(&format!("{base}/lwlauncher"), &format!("{base}/lwlauncher.md5"), &tmp, pr).and_then(|_| {
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &paths.exe).map_err(|e| e.to_string())
    });
    pr.set(PH_IDLE);
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map(|_| Some(version))
}

pub fn restart(paths: &Paths) -> ! {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(&paths.exe).args(std::env::args_os().skip(1)).exec();
    eprintln!("lwlauncher: restart failed: {err}");
    std::process::exit(1);
}

// LEGACY SCRIPTS
pub fn refresh_scripts(paths: &Paths, mut note: impl FnMut(String)) {
    let cache = paths.install.join(".scripts");
    for script in ["launcher.sh", "update.sh"] {
        let local = paths.install.join(script);
        if !local.exists() {
            continue;
        }
        let url = paths.url(script);
        let Ok((_, id)) = net::head(&url) else { continue };
        if id.is_empty() {
            continue;
        }
        let text = std::fs::read_to_string(&cache).unwrap_or_default();
        let prefix = format!("{script} ");
        if text.lines().any(|l| l.strip_prefix(&prefix) == Some(id.as_str())) {
            continue;
        }
        let tmp = paths.install.join(format!(".{script}.tmp"));
        if net::download(&url, &tmp, &AtomicU64::new(0)).is_err() {
            continue;
        }
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
        if std::fs::rename(&tmp, &local).is_ok() {
            let mut lines: Vec<String> = text.lines().filter(|l| !l.starts_with(&prefix)).map(String::from).collect();
            lines.push(format!("{script} {id}"));
            let _ = std::fs::write(&cache, lines.join("\n") + "\n");
            note(format!("{script}: updated"));
        }
    }
}

// DESKTOP ENTRY
pub fn write_desktop_entry(paths: &Paths) -> std::io::Result<()> {
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=LinusWare\nComment=LinusWare launcher\nExec=\"{}\"\nIcon=linusware\nTerminal=false\nCategories=Game;\nStartupWMClass=linusware\nX-LinusWare=lwlauncher\n",
        paths.exe.display()
    );
    let desktop = paths.desktop_file();
    if std::fs::read_to_string(&desktop).ok().as_deref() != Some(entry.as_str()) {
        std::fs::create_dir_all(desktop.parent().unwrap())?;
        std::fs::write(&desktop, entry)?;
    }
    let icon = paths.icon_file();
    if !icon.exists() {
        std::fs::create_dir_all(icon.parent().unwrap())?;
        std::fs::write(&icon, ICON_SVG)?;
    }
    Ok(())
}
