use crate::LinusWare;
use crate::LinusWare::net::Status;
use crate::LinusWare::{EDITION, Paths};
use minisign_verify::{PublicKey, Signature};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

pub const ICON_SVG: &str = include_str!("../../assets/linusware.svg");
const PUBKEY: &str = include_str!("../../assets/linusware.pub"); // minisign, key id 433C4F3A1BD878A0
const LAUNCHER_SH: &str = include_str!("../../scripts/launcher.sh");
const UPDATE_SH: &str = include_str!("../../scripts/update.sh");

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
    pub fn Set(&self, p: u8) {
        self.phase.store(p, Ordering::Relaxed);
    }
}

pub enum Outcome {
    UpToDate(String, String),
    Updated { from: Option<String>, to: String, log: String },
    NotReleased,
}

impl Outcome {
    pub fn Describe(&self) -> String {
        match self {
            Outcome::UpToDate(v, _) => format!("{EDITION}: already up-to-date (v{v})"),
            Outcome::Updated { from, to, .. } => format!("{EDITION}: {} -> v{to}", from.as_deref().unwrap_or("none")),
            Outcome::NotReleased => format!("{EDITION}: not updated yet"),
        }
    }
}

// SOURCES
fn StatusAt(paths: &Paths, what: &str) -> Result<(Status, String), String> {
    let mut err = String::from("no download source");
    for base in &paths.sources {
        match LinusWare::net::Fetch(&format!("{base}/{what}/status.json")) {
            Ok(text) => match LinusWare::net::Status::Parse(&text) {
                Some(st) => return Ok((st, base.clone())),
                None => err = "bad status.json".into(),
            },
            Err(e) => err = e,
        }
    }
    Err(err)
}

pub fn GetStatus(paths: &Paths, what: &str) -> Result<Status, String> {
    LinusWare::install::StatusAt(paths, what).map(|(st, _)| st)
}

fn Verify(file: &Path, sig: &Signature) -> Result<(), String> {
    use std::io::Read;
    let key = PublicKey::decode(PUBKEY).map_err(|e| e.to_string())?;
    let mut v = key.verify_stream(sig).map_err(|_| String::from("signature is from another key"))?;
    let mut f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        v.update(&buf[..n]);
    }
    v.finalize().map_err(|_| String::from("signature doesnt match the download"))
}

fn FetchVerified(url: &str, signed: &str, dest: &Path, pr: &Progress) -> Result<(), String> {
    let sig = Signature::decode(&LinusWare::net::Fetch(&format!("{url}.minisig"))?).map_err(|_| String::from("bad signature file"))?;
    if sig.trusted_comment() != signed {
        return Err(format!("signature isnt for '{signed}'"));
    }
    let total = LinusWare::net::Head(url).map(|(n, _)| n).unwrap_or(0);
    pr.total.store(total, Ordering::Relaxed);
    pr.got.store(0, Ordering::Relaxed);
    pr.Set(PH_DOWNLOAD);
    LinusWare::net::Download(url, dest, &pr.got)?;
    pr.Set(PH_VERIFY);
    let r = LinusWare::install::Verify(dest, &sig);
    if r.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    r
}

// EDITION
pub fn UpdateEdition(paths: &Paths, pr: &Progress, force: bool) -> Result<Outcome, String> {
    let ed = paths.ed.as_str();
    let (st, base) = LinusWare::install::StatusAt(paths, ed)?;
    if !st.updated {
        return Ok(Outcome::NotReleased);
    }
    let dir = paths.Edition();
    let cur = paths.InstalledVersion();
    if !force && paths.Installed() && cur.as_deref() == Some(st.version.as_str()) {
        return Ok(Outcome::UpToDate(st.version, st.log));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let archive = dir.join(".download.tmp");
    let result = (|| {
        LinusWare::install::FetchVerified(&format!("{base}/{ed}/bin/linusware.tar.gz"), &format!("{ed} {}", st.version), &archive, pr)?;
        pr.Set(PH_INSTALL);
        LinusWare::install::Unpack(&archive, &dir)?;
        LinusWare::install::WriteAtomic(&dir.join("version.txt"), format!("{}\n", st.version).as_bytes()).map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(&archive);
    pr.Set(PH_IDLE);
    result?;
    Ok(Outcome::Updated { from: cur, to: st.version, log: st.log })
}

fn Unpack(archive: &Path, dir: &Path) -> Result<(), String> {
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

fn WriteAtomic(p: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, p)
}

// SELF UPDATE
pub fn SelfUpdate(paths: &Paths, pr: &Progress) -> Result<Option<String>, String> {
    if !paths.ExeInstalled() {
        return Ok(None);
    }
    let Ok((st, base)) = LinusWare::install::StatusAt(paths, "launcher") else { return Ok(None) };
    if !st.updated || !LinusWare::net::Newer(&st.version, crate::LinusWare::VERSION) {
        return Ok(None);
    }
    let tmp = paths.install.join(".lwlauncher.new");
    let r = LinusWare::install::FetchVerified(&format!("{base}/launcher/bin/lwlauncher"), &format!("launcher {}", st.version), &tmp, pr).and_then(|_| {
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        LinusWare::install::Reports(&tmp, &st.version)?;
        std::fs::rename(&tmp, &paths.exe).map_err(|e| e.to_string())
    });
    pr.Set(PH_IDLE);
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map(|_| Some(st.version))
}

fn Reports(exe: &Path, want: &str) -> Result<(), String> {
    let out = std::process::Command::new(exe)
        .arg("--version")
        .env("LINUSWARE_DIR", exe.with_extension("check"))
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("new launcher: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let got = text.trim().trim_start_matches("lwlauncher").trim();
    if got == want.trim_start_matches('v') { Ok(()) } else { Err(format!("new launcher reports v{got}, expected v{want}")) }
}

pub fn Restart(paths: &Paths) -> ! {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(&paths.exe).args(std::env::args_os().skip(1)).exec();
    eprintln!("{}", LinusWare::Log::Line(&format!("Failed to restart: {err}")));
    std::process::exit(1);
}

// LEGACY SCRIPTS
pub fn RefreshScripts(paths: &Paths, mut note: impl FnMut(String)) {
    let _ = std::fs::remove_file(paths.install.join(".scripts"));
    for (script, text) in [("launcher.sh", LAUNCHER_SH), ("update.sh", UPDATE_SH)] {
        let local = paths.install.join(script);
        if !local.exists() || std::fs::read_to_string(&local).ok().as_deref() == Some(text) {
            continue;
        }
        let tmp = paths.install.join(format!(".{script}.tmp"));
        if std::fs::write(&tmp, text).is_err() {
            continue;
        }
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
        if std::fs::rename(&tmp, &local).is_ok() {
            note(format!("{script}: updated"));
        }
    }
}

// DESKTOP ENTRY
pub fn WriteDesktopEntry(paths: &Paths) -> std::io::Result<()> {
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=LinusWare\nComment=LinusWare launcher\nExec=\"{}\"\nIcon=linusware\nTerminal=false\nCategories=Game;\nStartupWMClass=linusware\nX-LinusWare=lwlauncher\n",
        paths.exe.display()
    );
    let desktop = paths.DesktopFile();
    if std::fs::read_to_string(&desktop).ok().as_deref() != Some(entry.as_str()) {
        std::fs::create_dir_all(desktop.parent().unwrap())?;
        std::fs::write(&desktop, entry)?;
    }
    let icon = paths.IconFile();
    if !icon.exists() {
        std::fs::create_dir_all(icon.parent().unwrap())?;
        std::fs::write(&icon, ICON_SVG)?;
    }
    Ok(())
}
