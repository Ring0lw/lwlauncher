use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};

pub struct Request {
    pub prompt: String,
    pub reply: Sender<Option<String>>,
}

// BRIDGE
pub struct Bridge {
    pub path: PathBuf,
    pub token: String,
}

fn random_hex() -> String {
    let mut b = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn runtime_dir() -> std::io::Result<PathBuf> {
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(d));
    }
    let d = std::env::temp_dir().join(format!("lwlauncher-{}", random_hex()));
    std::fs::create_dir(&d)?;
    std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700))?;
    Ok(d)
}

impl Bridge {
    pub fn start(mut on_request: impl FnMut(Request) + Send + 'static) -> std::io::Result<Bridge> {
        let path = runtime_dir()?.join(format!("lwlauncher-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let token = random_hex();
        let want = token.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut rd = BufReader::new(match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => continue,
                });
                let (mut tok, mut prompt) = (String::new(), String::new());
                if rd.read_line(&mut tok).is_err() || tok.trim() != want || rd.read_line(&mut prompt).is_err() {
                    continue;
                }
                let (tx, rx) = channel();
                on_request(Request { prompt: prompt.trim_end().to_string(), reply: tx });
                let mut stream = stream;
                if let Ok(Some(mut pw)) = rx.recv() {
                    pw.push('\n');
                    let _ = stream.write_all(pw.as_bytes());
                    pw.clear();
                }
            }
        });
        Ok(Bridge { path, token })
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// CLIENT
pub fn client_main(prompt: &str) -> i32 {
    if let (Some(path), Ok(token)) = (std::env::var_os("LW_ASKPASS"), std::env::var("LW_ASKPASS_TOKEN")) {
        if let Ok(mut s) = UnixStream::connect(path) {
            let _ = s.write_all(format!("{token}\n{}\n", prompt.replace('\n', " ")).as_bytes());
            let mut line = String::new();
            let ok = BufReader::new(s).read_line(&mut line).is_ok() && line.ends_with('\n');
            if ok {
                print!("{line}");
                let _ = std::io::stdout().flush();
                return 0;
            }
            return 1;
        }
    }
    match crate::app::run(crate::Paths::new(), crate::app::Mode::Askpass(prompt.to_string())) {
        Ok(code) => code,
        Err(_) => 1,
    }
}

pub fn user_from_prompt(prompt: &str) -> Option<String> {
    let i = prompt.find("password for ")? + "password for ".len();
    let u: String = prompt[i..].chars().take_while(|c| *c != ':' && !c.is_whitespace()).collect();
    (!u.is_empty()).then_some(u)
}
