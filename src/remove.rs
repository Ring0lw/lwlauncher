use crate::Paths;
use std::path::{Path, PathBuf};

const ENTRIES: [&str; 8] = ["external", "internal", "launcher.sh", "update.sh", ".scripts", "lwlauncher", "launcher.conf", ".lwlauncher.new"];
const MARKERS: [&str; 5] = ["external/run.sh", "external/version.txt", "launcher.sh", "update.sh", "lwlauncher"];

pub struct Plan {
    pub install: PathBuf,
    pub entries: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

impl Plan {
    pub fn all(&self) -> Vec<PathBuf> {
        self.entries.iter().chain(self.extra.iter()).cloned().collect()
    }
}

#[derive(Default)]
pub struct Report {
    pub failed: Vec<PathBuf>,
}

pub fn plan(paths: &Paths) -> Result<Plan, String> {
    let dir = paths.install.canonicalize().map_err(|_| "LinusWare is not installed".to_string())?;
    let home = paths.home.canonicalize().unwrap_or_else(|_| paths.home.clone());
    if dir == Path::new("/") || dir == home || !dir.is_dir() {
        return Err(format!("Refusing to delete {}", dir.display()));
    }
    if !MARKERS.iter().any(|m| std::fs::symlink_metadata(dir.join(m)).is_ok()) {
        return Err(format!("{} doesn't look like a LinusWare install", paths.pretty(&dir)));
    }
    if crate::launch::running(paths) {
        return Err("Close LinusWare before deleting it".into());
    }
    let entries = ENTRIES.iter().map(|e| dir.join(e)).filter(|p| std::fs::symlink_metadata(p).is_ok()).collect();
    let mut extra = Vec::new();
    let desktop = paths.desktop_file();
    if std::fs::read_to_string(&desktop).map(|s| s.contains("lwlauncher")).unwrap_or(false) {
        extra.push(desktop);
    }
    let icon = paths.icon_file();
    if icon.exists() {
        extra.push(icon);
    }
    Ok(Plan { install: dir, entries, extra })
}

fn remove_path(p: &Path) -> std::io::Result<()> {
    let m = std::fs::symlink_metadata(p)?;
    if m.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }
}

pub fn execute(plan: &Plan) -> Report {
    let mut r = Report::default();
    for p in plan.all() {
        if remove_path(&p).is_err() && std::fs::symlink_metadata(&p).is_ok() {
            r.failed.push(p);
        }
    }
    finish(plan);
    r
}

pub fn finish(plan: &Plan) {
    let empty = std::fs::read_dir(&plan.install).map(|mut d| d.next().is_none()).unwrap_or(false);
    if empty {
        let _ = std::fs::remove_dir(&plan.install);
    }
}

pub fn remove_config(paths: &Paths) -> Report {
    let mut r = Report::default();
    let dir = paths.config_dir();
    if remove_path(&dir).is_err() && std::fs::symlink_metadata(&dir).is_ok() {
        r.failed.push(dir);
    }
    r
}
