use crate::{webstore, Msg};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    os::windows::fs::OpenOptionsExt,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use tao::event_loop::EventLoopProxy;
use windows::{
    core::{w, PCWSTR},
    Win32::Networking::WinHttp::*,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const REPO: &str = "https://github.com/devifycode/azy-core-rust-browser";
const API: &str = "https://api.github.com/repos/devifycode/azy-core-rust-browser/releases/latest";
const MAX_EXE: usize = 128 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub platform: String,
    pub size: usize,
    pub sha256: String,
    pub url: String,
}
#[derive(Deserialize)]
struct Envelope {
    payload: String,
    signature: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub status: String,
    pub current: String,
    pub version: String,
    pub message: String,
    pub progress: u8,
}
impl Default for State {
    fn default() -> Self {
        Self {
            status: "idle".into(),
            current: env!("CARGO_PKG_VERSION").into(),
            version: String::new(),
            message: "Pembaruan diperiksa otomatis saat mulai dan setiap 6 jam.".into(),
            progress: 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Available {
    pub manifest: Manifest,
    pub envelope: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    target: PathBuf,
    root: PathBuf,
    version: String,
    original_sha256: String,
}
pub fn version(s: &str) -> Option<[u64; 3]> {
    let parts: Vec<_> = s.strip_prefix('v').unwrap_or(s).split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    Some([
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ])
}
pub fn verify_manifest(bytes: &[u8]) -> Result<Manifest> {
    if bytes.len() > 32768 {
        return Err("Manifest pembaruan terlalu besar.".into());
    }
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    let payload = STANDARD.decode(envelope.payload)?;
    let signature = STANDARD.decode(envelope.signature)?;
    webstore::verify_signature(
        include_bytes!("../assets/update-public.der"),
        &signature,
        &Sha256::digest(&payload),
        true,
    )
    .map_err(|_| "Tanda tangan pembaruan tidak valid.")?;
    let m: Manifest = serde_json::from_slice(&payload)?;
    if m.schema != 1
        || m.platform != "windows-x64"
        || version(&m.version).is_none()
        || m.version.starts_with('v')
        || m.size < 512
        || m.size > MAX_EXE
        || m.sha256.len() != 64
        || !m.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || m.url != format!("{REPO}/releases/download/v{}/AzyCore.exe", m.version)
    {
        return Err("Manifest pembaruan tidak sesuai Azy Core Windows x64.".into());
    }
    Ok(m)
}
fn verify_exe(bytes: &[u8], manifest: &Manifest) -> Result<()> {
    if bytes.len() != manifest.size
        || format!("{:x}", Sha256::digest(bytes)) != manifest.sha256.to_ascii_lowercase()
    {
        return Err("Checksum atau ukuran paket pembaruan tidak cocok.".into());
    }
    let pe = bytes
        .get(60..64)
        .map(|v| u32::from_le_bytes(v.try_into().unwrap()) as usize)
        .ok_or("Executable terpotong")?;
    if bytes.get(..2) != Some(b"MZ")
        || bytes.get(pe..pe.saturating_add(6)) != Some(b"PE\0\0\x64\x86")
    {
        return Err("Paket bukan executable Windows x64.".into());
    }
    Ok(())
}
struct Http(*mut std::ffi::c_void);
impl Drop for Http {
    fn drop(&mut self) {
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}
fn handle(p: *mut std::ffi::c_void) -> Result<Http> {
    if p.is_null() {
        Err(windows::core::Error::from_win32().into())
    } else {
        Ok(Http(p))
    }
}
fn trusted_url(input: &str) -> Result<url::Url> {
    let u = url::Url::parse(input)?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || !matches!(
            u.host_str(),
            Some(
                "github.com"
                    | "api.github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
    {
        return Err("Alamat pembaruan bukan endpoint HTTPS GitHub yang diizinkan.".into());
    }
    Ok(u)
}
fn get(input: &str, maximum: usize, mut progress: impl FnMut(usize)) -> Result<Vec<u8>> {
    let mut url = trusted_url(input)?;
    unsafe {
        let session = handle(WinHttpOpen(
            w!("AzyCore-Updater"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 10000, 10000, 20000, 20000)?;
        for _ in 0..6 {
            let host: Vec<u16> = url
                .host_str()
                .unwrap()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let connection = handle(WinHttpConnect(session.0, PCWSTR(host.as_ptr()), 443, 0))?;
            let endpoint = format!(
                "{}{}",
                url.path(),
                url.query().map(|q| format!("?{q}")).unwrap_or_default()
            );
            let path: Vec<u16> = endpoint.encode_utf16().chain(Some(0)).collect();
            let request = handle(WinHttpOpenRequest(
                connection.0,
                w!("GET"),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ))?;
            WinHttpSetOption(
                Some(request.0),
                WINHTTP_OPTION_REDIRECT_POLICY,
                Some(&WINHTTP_OPTION_REDIRECT_POLICY_NEVER.to_ne_bytes()),
            )?;
            WinHttpSendRequest(
                request.0,
                Some(
                    &"Accept: application/json\r\n"
                        .encode_utf16()
                        .collect::<Vec<_>>(),
                ),
                None,
                0,
                0,
                0,
            )?;
            WinHttpReceiveResponse(request.0, std::ptr::null_mut())?;
            let mut status = 0u32;
            let mut size = 4;
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&mut status as *mut u32).cast()),
                &mut size,
                std::ptr::null_mut(),
            )?;
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                let mut buffer = [0u16; 8192];
                let mut size = (buffer.len() * 2) as u32;
                WinHttpQueryHeaders(
                    request.0,
                    WINHTTP_QUERY_LOCATION,
                    PCWSTR::null(),
                    Some(buffer.as_mut_ptr().cast()),
                    &mut size,
                    std::ptr::null_mut(),
                )?;
                let location = String::from_utf16_lossy(
                    &buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())],
                );
                url = trusted_url(url.join(&location)?.as_str())?;
                continue;
            }
            if status != 200 {
                return Err(
                    format!("Server pembaruan merespons HTTP {status}. Coba lagi nanti.").into(),
                );
            }
            let start = Instant::now();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0u8; 65536];
                let mut read = 0;
                WinHttpReadData(
                    request.0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut read,
                )?;
                if read == 0 {
                    break;
                }
                if bytes.len() + read as usize > maximum
                    || start.elapsed() > Duration::from_secs(180)
                {
                    return Err("Unduhan pembaruan terlalu besar atau melewati batas waktu.".into());
                }
                bytes.extend_from_slice(&buffer[..read as usize]);
                progress(bytes.len());
            }
            return Ok(bytes);
        }
    }
    Err("Terlalu banyak pengalihan server pembaruan.".into())
}
pub fn check() -> Result<Option<Available>> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("AZY_CORE_UPDATE_FIXTURE") {
        let envelope = fs::read(PathBuf::from(path).join("update-manifest.json"))?;
        let manifest = verify_manifest(&envelope)?;
        return Ok(
            (version(&manifest.version) > version(env!("CARGO_PKG_VERSION")))
                .then_some(Available { manifest, envelope }),
        );
    }
    let release: serde_json::Value = serde_json::from_slice(&get(API, 1024 * 1024, |_| {})?)?;
    let tag = release["tag_name"]
        .as_str()
        .ok_or("Versi rilis tidak tersedia.")?;
    let latest = version(tag).ok_or("Versi rilis tidak valid.")?;
    if release["draft"].as_bool() != Some(false) || release["prerelease"].as_bool() != Some(false) {
        return Err("Rilis bukan versi stabil.".into());
    }
    if latest <= version(env!("CARGO_PKG_VERSION")).unwrap() {
        return Ok(None);
    }
    let assets = release["assets"]
        .as_array()
        .ok_or("Aset pembaruan tidak tersedia.")?;
    let asset = assets
        .iter()
        .find(|a| a["name"] == "update-manifest.json")
        .ok_or("Rilis terbaru belum memiliki paket pembaruan bertanda tangan.")?;
    let expected = format!("{REPO}/releases/download/{tag}/update-manifest.json");
    if asset["browser_download_url"].as_str() != Some(expected.as_str()) {
        return Err("Lokasi manifest rilis tidak valid.".into());
    }
    let envelope = get(&expected, 32768, |_| {})?;
    let manifest = verify_manifest(&envelope)?;
    if version(&manifest.version) != Some(latest) {
        return Err("Versi manifest tidak sesuai rilis terbaru.".into());
    }
    Ok(Some(Available { manifest, envelope }))
}
pub fn download(
    root: &Path,
    available: &Available,
    proxy: &EventLoopProxy<Msg>,
) -> Result<PathBuf> {
    let mut last = 0;
    #[cfg(debug_assertions)]
    let fixture = std::env::var_os("AZY_CORE_UPDATE_FIXTURE");
    #[cfg(not(debug_assertions))]
    let fixture: Option<std::ffi::OsString> = None;
    let bytes = if let Some(fixture) = fixture {
        fs::read(PathBuf::from(fixture).join("AzyCore.exe"))?
    } else {
        get(&available.manifest.url, available.manifest.size, |size| {
            let value = ((size * 100) / available.manifest.size).min(100) as u8;
            if value != last {
                last = value;
                let _ = proxy.send_event(Msg::UpdateProgress(value));
            }
        })?
    };
    verify_exe(&bytes, &available.manifest)?;
    let target = std::env::current_exe()?;
    if target
        .file_name()
        .is_none_or(|name| !name.to_string_lossy().eq_ignore_ascii_case("AzyCore.exe"))
    {
        return Err("Jalankan executable bernama AzyCore.exe untuk memperbarui aplikasi.".into());
    }
    let folder = root.join("Updates").join(format!(
        "{}-{}",
        available.manifest.version,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(&folder)?;
    // Probe destination permissions before offering a restart. Never overwrite files.
    let mut probe = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target.with_file_name(format!(".azy-update-probe-{}.tmp", std::process::id())))?;
    probe.write_all(b"Azy Core update probe")?;
    drop(probe);
    fs::remove_file(
        target.with_file_name(format!(".azy-update-probe-{}.tmp", std::process::id())),
    )?;
    fs::write(folder.join("AzyCore.exe"), bytes)?;
    fs::write(folder.join("update-manifest.json"), &available.envelope)?;
    fs::copy(&target, folder.join("update-helper.exe"))?;
    fs::write(
        folder.join("plan.json"),
        serde_json::to_vec(&Plan {
            target: target.clone(),
            root: root.to_path_buf(),
            version: available.manifest.version.clone(),
            original_sha256: format!("{:x}", Sha256::digest(fs::read(&target)?)),
        })?,
    )?;
    Ok(folder.join("plan.json"))
}
pub fn install(plan: &Path) -> Result<()> {
    // Reverify after downloading, including edits made while waiting to restart.
    let (validated, _) = validate_plan(plan)?;
    if format!(
        "{:x}",
        Sha256::digest(fs::read(plan.with_file_name("update-helper.exe"))?)
    ) != validated.original_sha256
    {
        return Err("Program pembaruan telah berubah. Unduh pembaruan kembali.".into());
    }
    Command::new(plan.with_file_name("update-helper.exe"))
        .arg("--apply-update")
        .arg(plan)
        .creation_flags(0x08000000)
        .spawn()?;
    Ok(())
}
fn validate_plan(path: &Path) -> Result<(Plan, Manifest)> {
    let plan: Plan = serde_json::from_slice(&fs::read(path)?)?;
    let folder = path
        .parent()
        .ok_or("Folder pembaruan tidak valid")?
        .canonicalize()?;
    let updates = plan.root.join("Updates").canonicalize()?;
    if folder.parent() != Some(updates.as_path())
        || !plan.target.is_absolute()
        || plan
            .target
            .file_name()
            .is_none_or(|n| !n.to_string_lossy().eq_ignore_ascii_case("AzyCore.exe"))
    {
        return Err("Lokasi pemasangan pembaruan tidak valid.".into());
    }
    let manifest = verify_manifest(&fs::read(folder.join("update-manifest.json"))?)?;
    if manifest.version != plan.version
        || version(&manifest.version) <= version(env!("CARGO_PKG_VERSION"))
    {
        return Err("Versi pembaruan harus lebih baru.".into());
    }
    verify_exe(&fs::read(folder.join("AzyCore.exe"))?, &manifest)?;
    Ok((plan, manifest))
}
fn apply(path: &Path) -> Result<()> {
    let (plan, manifest) = validate_plan(path)?;
    let folder = path.parent().unwrap();
    if format!("{:x}", Sha256::digest(fs::read(&plan.target)?)) != plan.original_sha256 {
        return Err(
            "Executable telah berubah sejak pembaruan diunduh. Periksa pembaruan kembali.".into(),
        );
    }
    let replacement = plan
        .target
        .with_file_name(format!(".azy-update-{}.tmp", std::process::id()));
    let backup = plan
        .target
        .with_file_name(format!(".azy-update-{}.bak", std::process::id()));
    let mut copy = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&replacement)?;
    copy.write_all(&fs::read(folder.join("AzyCore.exe"))?)?;
    copy.sync_all()?;
    drop(copy);
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&plan.target)
        {
            Ok(file) => {
                drop(file);
                break;
            }
            Err(error) => {
                if Instant::now() >= deadline {
                    let _ = fs::remove_file(&replacement);
                    return Err(format!(
                        "Tutup jendela Azy Core lain sebelum memperbarui: {error}"
                    )
                    .into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    let result = (|| -> Result<()> {
        fs::rename(&plan.target, &backup)?;
        if let Err(error) = fs::rename(&replacement, &plan.target) {
            let _ = fs::rename(&backup, &plan.target);
            return Err(error.into());
        }
        if let Err(error) = verify_exe(&fs::read(&plan.target)?, &manifest) {
            let _ = fs::remove_file(&plan.target);
            let _ = fs::rename(&backup, &plan.target);
            return Err(error);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&replacement);
        return result;
    }
    let receipt = serde_json::json!({"version":manifest.version,"message":"Pembaruan berhasil dipasang.","time":crate::model::now()});
    let _ = fs::write(
        plan.root.join("update-result.json"),
        serde_json::to_vec(&receipt)?,
    );
    let mut restart = Command::new(&plan.target);
    restart
        .arg("--restore-after-update")
        .env("AZY_CORE_DATA_DIR", &plan.root);
    if let Err(error) = restart.spawn() {
        let _ = fs::remove_file(&plan.target);
        let _ = fs::rename(&backup, &plan.target);
        return Err(error.into());
    }
    update_install_metadata(&plan, &manifest);
    let _ = fs::remove_file(&backup);
    // Only remove exact files owned by this update job, after the replacement starts.
    let _ = fs::remove_file(folder.join("AzyCore.exe"));
    Ok(())
}
fn update_install_metadata(plan: &Plan, manifest: &Manifest) {
    let Some(directory) = plan.target.parent() else {
        return;
    };
    let metadata = directory.join("AzyCore.BUILD-INFO.json");
    if metadata.is_file() {
        let value = serde_json::json!({"application":"Azy Core","version":manifest.version,
            "bytes":manifest.size,"sha256":manifest.sha256,"updatedViaBrowser":true});
        if let Ok(bytes) = serde_json::to_vec_pretty(&value) {
            let _ = fs::write(metadata, bytes);
        }
    }
    // Update only an existing installer registration for this exact directory.
    unsafe {
        use windows::Win32::System::Registry::*;
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\AzyCore"),
            None,
            KEY_READ | KEY_WRITE | KEY_WOW64_64KEY,
            &mut key,
        )
        .is_err()
        {
            return;
        }
        let mut buffer = [0u16; 32768];
        let mut length = (buffer.len() * 2) as u32;
        let read = RegQueryValueExW(
            key,
            w!("InstallLocation"),
            None,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut length),
        );
        if read.is_ok() {
            let location = String::from_utf16_lossy(
                &buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())],
            );
            let same = fs::canonicalize(&location)
                .ok()
                .zip(fs::canonicalize(directory).ok())
                .is_some_and(|(a, b)| {
                    a.to_string_lossy()
                        .eq_ignore_ascii_case(&b.to_string_lossy())
                });
            if same {
                let value: Vec<u8> = manifest
                    .version
                    .encode_utf16()
                    .chain(Some(0))
                    .flat_map(u16::to_le_bytes)
                    .collect();
                let _ = RegSetValueExW(key, w!("DisplayVersion"), None, REG_SZ, Some(&value));
            }
        }
        let _ = RegCloseKey(key);
    }
}
pub fn cleanup(root: &Path) {
    let Ok(base) = root.join("Updates").canonicalize() else {
        return;
    };
    let Ok(entries) = fs::read_dir(root.join("Updates")) else {
        return;
    };
    for entry in entries.flatten() {
        let folder = entry.path();
        if folder
            .canonicalize()
            .ok()
            .is_none_or(|p| p.parent() != Some(base.as_path()))
        {
            continue;
        }
        let Ok(bytes) = fs::read(folder.join("plan.json")) else {
            continue;
        };
        let Ok(plan) = serde_json::from_slice::<Plan>(&bytes) else {
            continue;
        };
        let Ok(envelope) = fs::read(folder.join("update-manifest.json")) else {
            continue;
        };
        if verify_manifest(&envelope).is_err() {
            continue;
        }
        if version(&plan.version).is_none_or(|v| v > version(env!("CARGO_PKG_VERSION")).unwrap()) {
            continue;
        }
        if fs::canonicalize(&plan.root).ok() != fs::canonicalize(root).ok() {
            continue;
        }
        for name in ["AzyCore.exe", "update-helper.exe"] {
            let _ = fs::remove_file(folder.join(name));
        }
        if !folder.join("update-helper.exe").exists() {
            for name in ["update-manifest.json", "plan.json"] {
                let _ = fs::remove_file(folder.join(name));
            }
            let _ = fs::remove_dir(folder);
        }
    }
}
pub fn helper(args: &[String]) -> Option<std::result::Result<(), String>> {
    if args.first().map(String::as_str) != Some("--apply-update") {
        return None;
    }
    let result = args
        .get(1)
        .ok_or("Rencana pembaruan tidak tersedia.".to_string())
        .and_then(|path| apply(Path::new(path)).map_err(|e| e.to_string()));
    if let Err(error) = &result {
        // Keep the original browser usable and explain a failed replacement.
        if let Some(path) = args.get(1) {
            if let Ok((plan, _)) = validate_plan(Path::new(path)) {
                let _ = fs::write(
                    plan.root.join("update-result.json"),
                    serde_json::to_vec(&serde_json::json!({"message":error,"failed":true}))
                        .unwrap_or_default(),
                );
                let _ = Command::new(plan.target)
                    .arg("--restore-after-update")
                    .env("AZY_CORE_DATA_DIR", plan.root)
                    .spawn();
            }
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_are_numeric_and_stable_only() {
        assert!(version("v0.1.10") > version("0.1.9"));
        for value in ["1.0", "1.0.0-beta", "1.0.0.1", "1.-1.0", "1..0", "v"] {
            assert!(version(value).is_none());
        }
    }
    #[test]
    fn update_hosts_require_trusted_https() {
        assert!(trusted_url(API).is_ok());
        for value in [
            "http://github.com/file",
            "https://github.com.evil.test/file",
            "https://user@github.com/file",
            "file:///C:/file",
            "https://github.com:444/file",
        ] {
            assert!(trusted_url(value).is_err());
        }
    }
    #[test]
    fn unsigned_manifests_are_rejected() {
        assert!(verify_manifest(br#"{"payload":"e30=","signature":"AA=="}"#).is_err());
        assert!(verify_manifest(&vec![0; 32769]).is_err());
    }
    #[test]
    fn signed_manifest_detects_modified_payload() {
        let bytes = include_bytes!("../assets/tests/update-manifest.json");
        assert_eq!(verify_manifest(bytes).unwrap().version, "0.0.1");
        let mut envelope: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let mut payload = STANDARD
            .decode(envelope["payload"].as_str().unwrap())
            .unwrap();
        payload[0] ^= 1;
        envelope["payload"] = STANDARD.encode(payload).into();
        assert!(verify_manifest(&serde_json::to_vec(&envelope).unwrap()).is_err());
    }
    #[test]
    #[ignore = "Run after publishing the signed release channel"]
    fn official_release_package_verifies_over_https() {
        let release: serde_json::Value =
            serde_json::from_slice(&get(API, 1024 * 1024, |_| {}).unwrap()).unwrap();
        let tag = release["tag_name"].as_str().unwrap();
        let envelope = get(
            &format!("{REPO}/releases/download/{tag}/update-manifest.json"),
            32768,
            |_| {},
        )
        .unwrap();
        let manifest = verify_manifest(&envelope).unwrap();
        assert_eq!(version(&manifest.version), version(tag));
        let bytes = get(&manifest.url, manifest.size, |_| {}).unwrap();
        verify_exe(&bytes, &manifest).unwrap();
    }
}
