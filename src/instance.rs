//! Per-profile launch broker. Secondary launches never read or overwrite browser.json.
use crate::Msg;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tao::event_loop::EventLoopProxy;
use windows::Win32::{
    Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
    UI::WindowsAndMessaging::AllowSetForegroundWindow,
};

#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    token: String,
    pid: u32,
}
#[derive(Serialize, Deserialize)]
struct Request {
    token: String,
    id: String,
    inputs: Vec<String>,
    private: bool,
}
pub struct Instance {
    lock: Option<File>,
    lock_path: PathBuf,
    endpoint: PathBuf,
}
impl Drop for Instance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.endpoint);
        drop(self.lock.take());
        let _ = fs::remove_file(&self.lock_path);
    }
}
pub enum Launch {
    Primary(Instance),
    Forwarded,
}
fn nonce() -> io::Result<String> {
    let mut bytes = [0; 32];
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(io::Error::other)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn forward(endpoint: &Path, request: &mut Request) -> io::Result<()> {
    let endpoint: Endpoint = serde_json::from_slice(&fs::read(endpoint)?)?;
    let mut stream = TcpStream::connect_timeout(
        &SocketAddrV4::new(Ipv4Addr::LOCALHOST, endpoint.port).into(),
        Duration::from_millis(300),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    request.token = endpoint.token;
    // Explorer starts the sender in the foreground; grant that right to the receiver.
    let _ = unsafe { AllowSetForegroundWindow(endpoint.pid) };
    serde_json::to_writer(&mut stream, request)?;
    stream.write_all(b"\n")?;
    let mut reply = [0; 2];
    stream.read_exact(&mut reply)?;
    if &reply != b"OK" {
        return Err(io::Error::other("Azy Core menolak permintaan buka file."));
    }
    Ok(())
}
fn locked(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32 | 33))
}
fn exclusive(path: &Path, create: bool) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .share_mode(0)
        .open(path)
}
pub fn has_other_windows(root: &Path) -> bool {
    fs::read_dir(root.join("Launch"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter(|entry| {
            exclusive(&entry.path().with_extension("lock"), false).is_err_and(|e| locked(&e))
        })
        .take(2)
        .count()
        > 1
}
pub fn start(root: &Path, args: &[String], proxy: EventLoopProxy<Msg>) -> io::Result<Launch> {
    let inputs: Vec<_> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .take(32)
        .cloned()
        .collect();
    let reuse = !inputs.is_empty() && !args.iter().any(|a| a == "--new-window");
    // Serialize discovery/registration so simultaneous Explorer launches cannot
    // both miss the other process while its window is still starting.
    let deadline = Instant::now() + Duration::from_secs(15);
    let _startup_lock = loop {
        match exclusive(&root.join("launch.lock"), true) {
            Ok(lock) => break lock,
            Err(error) if locked(&error) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(error) => return Err(error),
        }
    };
    let brokers = root.join("Launch");
    fs::create_dir_all(&brokers)?;
    let id = nonce()?;
    let endpoint_path = brokers.join(format!("{id}.json"));
    let lock_path = endpoint_path.with_extension("lock");
    let mut request = Request {
        token: String::new(),
        id,
        inputs,
        private: args.iter().any(|a| a == "--private"),
    };
    loop {
        let mut active = false;
        for entry in fs::read_dir(&brokers)?.flatten() {
            let endpoint = entry.path();
            if endpoint.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let lock_path = endpoint.with_extension("lock");
            match exclusive(&lock_path, false) {
                Err(error) if locked(&error) => {
                    active = true;
                    if reuse && forward(&endpoint, &mut request).is_ok() {
                        return Ok(Launch::Forwarded);
                    }
                }
                Ok(stale_lock) => {
                    drop(stale_lock);
                    let _ = fs::remove_file(&endpoint);
                    let _ = fs::remove_file(&lock_path);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let _ = fs::remove_file(&endpoint);
                }
                Err(_) => {}
            }
        }
        if !reuse || !active {
            break;
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(
                "Azy Core yang sedang terbuka belum merespons. Coba buka file kembali.",
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let lock = exclusive(&lock_path, true)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let token = nonce()?;
    let endpoint = Endpoint {
        port: listener.local_addr()?.port(),
        token: token.clone(),
        pid: std::process::id(),
    };
    fs::write(&endpoint_path, serde_json::to_vec(&endpoint)?)?;
    std::thread::Builder::new()
        .name("azy-launch-broker".into())
        .spawn(move || {
            let mut received = VecDeque::new();
            for mut stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let mut line = String::new();
                if BufReader::new((&mut stream).take(65_537))
                    .read_line(&mut line)
                    .is_err()
                    || line.len() > 65_536
                {
                    continue;
                }
                let Ok(request) = serde_json::from_str::<Request>(&line) else {
                    continue;
                };
                if request.token != token || request.inputs.is_empty() || request.inputs.len() > 32
                {
                    continue;
                }
                if !received.contains(&request.id) {
                    if proxy
                        .send_event(Msg::ExternalOpen(request.inputs, request.private))
                        .is_err()
                    {
                        break;
                    }
                    received.push_back(request.id);
                    if received.len() > 128 {
                        received.pop_front();
                    }
                }
                let _ = stream.write_all(b"OK");
            }
        })?;
    Ok(Launch::Primary(Instance {
        lock: Some(lock),
        lock_path,
        endpoint: endpoint_path,
    }))
}
