use crate::LinusWare;
use crate::LinusWare::Paths;
use std::path::{Path, PathBuf};

const ENTRIES: [&str; 9] = ["external", "cs2", "internal", "launcher.sh", "update.sh", ".scripts", "lwlauncher", "launcher.conf", ".lwlauncher.new"];
const MARKERS: [&str; 6] = ["external/run.sh", "external/version.txt", "cs2/run.sh", "launcher.sh", "update.sh", "lwlauncher"];

pub struct Plan {
    pub install: PathBuf,
    pub entries: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

impl Plan {
    pub fn All(&self) -> Vec<PathBuf> {
        self.entries.iter().chain(self.extra.iter()).cloned().collect()
    }
}

#[derive(Default)]
pub struct Report {
    pub failed: Vec<PathBuf>,
}

pub fn MakePlan(paths: &Paths) -> Result<Plan, String> {
    let dir = paths.install.canonicalize().map_err(|_| "LinusWare is not installed".to_string())?;
    let home = paths.home.canonicalize().unwrap_or_else(|_| paths.home.clone());
    if dir == Path::new("/") || dir == home || !dir.is_dir() {
        return Err(format!("Refusing to delete {}", dir.display()));
    }
    if !MARKERS.iter().any(|m| std::fs::symlink_metadata(dir.join(m)).is_ok()) {
        return Err(format!("{} doesn't look like a LinusWare install", paths.Pretty(&dir)));
    }
    if LinusWare::launch::IsRunning(paths) {
        return Err("Close LinusWare before deleting it".into());
    }
    let entries = ENTRIES.iter().map(|e| dir.join(e)).filter(|p| std::fs::symlink_metadata(p).is_ok()).collect();
    let mut extra = Vec::new();
    let desktop = paths.DesktopFile();
    if std::fs::read_to_string(&desktop).map(|s| s.contains("lwlauncher")).unwrap_or(false) {
        extra.push(desktop);
    }
    let lw = paths.home.join(".local/bin/lw");
    if std::fs::read_link(&lw).map(|t| t.starts_with(&dir) || t.starts_with(&paths.install)).unwrap_or(false) {
        extra.push(lw);
    }
    let icon = paths.IconFile();
    if icon.exists() {
        extra.push(icon);
    }
    Ok(Plan { install: dir, entries, extra })
}

fn RemovePath(p: &Path) -> std::io::Result<()> {
    let m = std::fs::symlink_metadata(p)?;
    if m.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }
}

pub fn Execute(plan: &Plan) -> Report {
    let mut r = Report::default();
    for p in plan.All() {
        if LinusWare::remove::RemovePath(&p).is_err() && std::fs::symlink_metadata(&p).is_ok() {
            r.failed.push(p);
        }
    }
    LinusWare::remove::Finish(plan);
    r
}

pub fn Finish(plan: &Plan) {
    let empty = std::fs::read_dir(&plan.install).map(|mut d| d.next().is_none()).unwrap_or(false);
    if empty {
        let _ = std::fs::remove_dir(&plan.install);
    }
}

pub fn RemoveConfig(paths: &Paths) -> Report {
    let mut r = Report::default();
    let dir = paths.ConfigDir();
    if LinusWare::remove::RemovePath(&dir).is_err() && std::fs::symlink_metadata(&dir).is_ok() {
        r.failed.push(dir);
    }
    r
}
