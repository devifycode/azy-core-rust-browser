use crate::{extensions, model::ExtensionSource};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Networking::WinHttp::*,
        Security::Cryptography::*,
    },
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const MAX_PACKAGE: usize = 100 * 1024 * 1024;
// Chromium's production Chrome Web Store publisher SPKI hash (crx_verifier.cc).
const PUBLISHER: [u8; 32] = [
    0x61, 0xf7, 0xf2, 0xa6, 0xbf, 0xcf, 0x74, 0xcd, 0x0b, 0xc1, 0xfe, 0x24, 0x97, 0xcc, 0x9b, 0x04,
    0x25, 0x4c, 0x65, 0x8f, 0x79, 0xf2, 0x14, 0x53, 0x92, 0x86, 0x7e, 0xa8, 0x36, 0x63, 0x67, 0xcf,
];
#[derive(Clone, Debug, serde::Serialize)]
pub struct Prepared {
    pub id: String,
    pub name: String,
    pub source: ExtensionSource,
}
pub fn store_id(input: &str) -> Option<String> {
    let input = input.trim();
    if input.len() == 32 && input.bytes().all(|b| (b'a'..=b'p').contains(&b)) {
        return Some(input.into());
    }
    let url = url::Url::parse(input).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    let path = url.path();
    if !matches!(
        url.host_str(),
        Some("chromewebstore.google.com" | "chrome.google.com")
    ) || !(path.starts_with("/detail/") || path.starts_with("/webstore/detail/"))
    {
        return None;
    }
    let id = path.trim_end_matches('/').rsplit('/').next()?;
    (id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))).then(|| id.into())
}
pub fn prepare(root: &Path, id: &str, version: &str) -> Result<Prepared> {
    let id = store_id(id).ok_or("ID Chrome Web Store tidak valid")?;
    let mut url = url::Url::parse("https://clients2.google.com/service/update2/crx")?;
    url.query_pairs_mut().extend_pairs([
        ("response", "redirect"),
        ("os", "win"),
        ("arch", "x64"),
        ("prod", "chromecrx"),
        ("prodversion", version),
        ("acceptformat", "crx3"),
        ("x", &format!("id={id}&installsource=ondemand&uc")),
    ]);
    prepare_bytes(root, &id, &download(&url)?)
}
pub fn prepare_bytes(root: &Path, id: &str, bytes: &[u8]) -> Result<Prepared> {
    let (archive, key) = verify_crx(bytes, id)?;
    let base = root.join("Extensions");
    fs::create_dir_all(&base)?;
    let base = base.canonicalize()?;
    let hash: String = Sha256::digest(bytes)[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let folder = base.join(format!("{id}-{hash}-{stamp}"));
    fs::create_dir(&folder)?;
    let result = (|| {
        extract_zip(archive, &folder)?;
        let manifest_file = folder.join("manifest.json");
        if fs::metadata(&manifest_file)?.len() > 1024 * 1024 {
            return Err("Manifest terlalu besar".into());
        }
        let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_file)?)?;
        manifest["key"] = base64::engine::general_purpose::STANDARD.encode(key).into();
        fs::write(&manifest_file, serde_json::to_vec_pretty(&manifest)?)?;
        let name = localized_name(&manifest, &folder);
        let mut source = extensions::read_source(&folder)?;
        source.store_id = Some(id.into());
        Ok(Prepared {
            id: id.into(),
            name,
            source,
        })
    })();
    // This newly created directory is always below the resolved managed root.
    if result.is_err() {
        if let Ok(resolved) = folder.canonicalize() {
            if resolved.parent() == Some(base.as_path()) {
                let _ = fs::remove_dir_all(resolved);
            }
        }
    }
    result
}
pub fn discard(root: &Path, package: &Prepared) {
    if let (Ok(base), Ok(folder)) = (
        root.join("Extensions").canonicalize(),
        Path::new(&package.source.folder).canonicalize(),
    ) {
        if folder.parent() == Some(base.as_path())
            && folder.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with(&format!("{}-", package.id))
            })
        {
            let _ = fs::remove_dir_all(folder);
        }
    }
}
fn localized_name(manifest: &serde_json::Value, folder: &Path) -> String {
    let name = manifest["name"].as_str().unwrap_or("Ekstensi");
    let Some(key) = name
        .strip_prefix("__MSG_")
        .and_then(|s| s.strip_suffix("__"))
    else {
        return name.into();
    };
    let locale = manifest["default_locale"].as_str().unwrap_or("en");
    if !locale
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return name.into();
    }
    let file = folder.join("_locales").join(locale).join("messages.json");
    if fs::metadata(&file)
        .map(|m| m.len() > 4 * 1024 * 1024)
        .unwrap_or(true)
    {
        return name.into();
    }
    fs::read(file)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| {
            v.as_object()?
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))?
                .1["message"]
                .as_str()
                .map(String::from)
        })
        .unwrap_or_else(|| name.into())
}
struct Http(*mut std::ffi::c_void);
impl Drop for Http {
    fn drop(&mut self) {
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}
fn http(handle: *mut std::ffi::c_void) -> Result<Http> {
    if handle.is_null() {
        Err(windows::core::Error::from_win32().into())
    } else {
        Ok(Http(handle))
    }
}
fn download(url: &url::Url) -> Result<Vec<u8>> {
    unsafe {
        let session = http(WinHttpOpen(
            w!("AzyCore/0.1"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 15000, 15000, 30000, 30000)?;
        let host: Vec<u16> = url
            .host_str()
            .ok_or("Host tidak valid")?
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let connection = http(WinHttpConnect(session.0, PCWSTR(host.as_ptr()), 443, 0))?;
        let path: Vec<u16> = format!("{}?{}", url.path(), url.query().unwrap_or(""))
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let request = http(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0)?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())?;
        let mut status = 0u32;
        let mut length = 4;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut length,
            std::ptr::null_mut(),
        )?;
        if status != 200 {
            return Err(format!("Chrome Web Store tidak menyediakan paket (HTTP {status}). Ekstensi mungkin dibatasi atau memerlukan akun.").into());
        }
        let mut bytes = Vec::new();
        let start = Instant::now();
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
            if bytes.len() + read as usize > MAX_PACKAGE
                || start.elapsed() > Duration::from_secs(180)
            {
                return Err("Paket terlalu besar atau unduhan melewati batas waktu".into());
            }
            bytes.extend_from_slice(&buffer[..read as usize]);
        }
        Ok(bytes)
    }
}
fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        data.get(offset..offset + 2)
            .ok_or("Paket terpotong")?
            .try_into()?,
    ))
}
fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or("Paket terpotong")?
            .try_into()?,
    ))
}
fn varint(data: &[u8], offset: &mut usize) -> Result<u64> {
    let mut value = 0;
    for shift in (0..70).step_by(7) {
        let byte = *data.get(*offset).ok_or("Header protobuf terpotong")?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err("Varint tidak valid".into());
        }
        value |= ((byte & 127) as u64) << shift;
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    Err("Varint terlalu panjang".into())
}
fn fields(data: &[u8]) -> Result<Vec<(u64, &[u8])>> {
    let mut offset = 0;
    let mut result = Vec::new();
    while offset < data.len() {
        let tag = varint(data, &mut offset)?;
        if tag >> 3 == 0 {
            return Err("Field protobuf tidak valid".into());
        }
        match tag & 7 {
            0 => {
                varint(data, &mut offset)?;
            }
            1 => offset = offset.checked_add(8).ok_or("Header overflow")?,
            5 => offset = offset.checked_add(4).ok_or("Header overflow")?,
            2 => {
                let len = usize::try_from(varint(data, &mut offset)?)?;
                let end = offset.checked_add(len).ok_or("Header overflow")?;
                let bytes = data.get(offset..end).ok_or("Header protobuf terpotong")?;
                result.push((tag >> 3, bytes));
                offset = end;
            }
            _ => return Err("Tipe protobuf tidak didukung".into()),
        }
        if offset > data.len() {
            return Err("Header terpotong".into());
        }
    }
    Ok(result)
}
fn verify_crx<'a>(bytes: &'a [u8], expected: &str) -> Result<(&'a [u8], Vec<u8>)> {
    if bytes.len() > MAX_PACKAGE || bytes.get(..4) != Some(b"Cr24") || u32_at(bytes, 4)? != 3 {
        return Err("Paket bukan CRX3 yang didukung".into());
    }
    let length = u32_at(bytes, 8)? as usize;
    if length > 1024 * 1024 {
        return Err("Header CRX terlalu besar".into());
    }
    let end = 12usize.checked_add(length).ok_or("Header overflow")?;
    let header = fields(bytes.get(12..end).ok_or("Header terpotong")?)?;
    let archive = bytes.get(end..).ok_or("Arsip terpotong")?;
    let signed = header
        .iter()
        .find(|f| f.0 == 10000)
        .ok_or("Header tanda tangan hilang")?
        .1;
    let id = fields(signed)?
        .into_iter()
        .find(|f| f.0 == 1)
        .ok_or("ID CRX hilang")?
        .1;
    if id.len() != 16
        || id
            .iter()
            .flat_map(|b| [b'a' + (b >> 4), b'a' + (b & 15)])
            .map(char::from)
            .collect::<String>()
            != expected
    {
        return Err("ID paket berbeda dari Chrome Web Store yang diminta".into());
    }
    let mut hasher = Sha256::new();
    hasher.update(b"CRX3 SignedData\0");
    hasher.update((signed.len() as u32).to_le_bytes());
    hasher.update(signed);
    hasher.update(archive);
    let hash = hasher.finalize();
    let mut developer = None;
    let mut publisher = false;
    let mut proofs = 0;
    for (kind, proof) in header.into_iter().filter(|f| f.0 == 2 || f.0 == 3) {
        proofs += 1;
        if proofs > 16 {
            return Err("Terlalu banyak tanda tangan".into());
        }
        let proof = fields(proof)?;
        let key = proof.iter().find(|f| f.0 == 1).ok_or("Kunci hilang")?.1;
        let signature = proof
            .iter()
            .find(|f| f.0 == 2)
            .ok_or("Tanda tangan hilang")?
            .1;
        verify_signature(key, signature, &hash, kind == 2)?;
        let key_hash = Sha256::digest(key);
        if key_hash[..16] == *id {
            developer = Some(key.to_vec())
        }
        if key_hash[..] == PUBLISHER {
            publisher = true
        }
    }
    if !publisher {
        return Err("Tanda tangan penerbit Chrome Web Store tidak ditemukan".into());
    }
    Ok((
        archive,
        developer.ok_or("Tanda tangan pengembang yang sesuai ID tidak ditemukan")?,
    ))
}
fn ecdsa_raw(der: &[u8]) -> Result<Vec<u8>> {
    if der.len() < 6 || der[0] != 0x30 || der[1] as usize + 2 != der.len() {
        return Err("Tanda tangan ECDSA tidak valid".into());
    }
    let mut out = Vec::with_capacity(64);
    let mut offset = 2;
    for _ in 0..2 {
        if der.get(offset) != Some(&2) {
            return Err("Integer ECDSA tidak valid".into());
        }
        let len = *der.get(offset + 1).ok_or("Integer terpotong")? as usize;
        let mut value = der
            .get(offset + 2..offset + 2 + len)
            .ok_or("Integer terpotong")?;
        offset += 2 + len;
        if value.len() == 33 && value[0] == 0 {
            value = &value[1..]
        }
        if value.len() > 32 {
            return Err("Integer ECDSA terlalu besar".into());
        }
        out.resize(out.len() + 32 - value.len(), 0);
        out.extend_from_slice(value)
    }
    if offset != der.len() {
        return Err("Sisa tanda tangan tidak valid".into());
    }
    Ok(out)
}
pub(crate) fn verify_signature(
    public: &[u8],
    signature: &[u8],
    hash: &[u8],
    rsa: bool,
) -> Result<()> {
    unsafe {
        let mut info: *mut CERT_PUBLIC_KEY_INFO = std::ptr::null_mut();
        let mut length = 0;
        CryptDecodeObjectEx(
            X509_ASN_ENCODING,
            X509_PUBLIC_KEY_INFO,
            public,
            CRYPT_DECODE_ALLOC_FLAG,
            None,
            Some((&mut info as *mut *mut CERT_PUBLIC_KEY_INFO).cast()),
            &mut length,
        )?;
        let mut key = BCRYPT_KEY_HANDLE::default();
        let result = CryptImportPublicKeyInfoEx2(
            X509_ASN_ENCODING,
            info,
            CRYPT_IMPORT_PUBLIC_KEY_FLAGS(0),
            None,
            &mut key,
        );
        let _ = LocalFree(Some(HLOCAL(info.cast())));
        result?;
        let padding = BCRYPT_PKCS1_PADDING_INFO {
            pszAlgId: BCRYPT_SHA256_ALGORITHM,
        };
        let result = if rsa {
            BCryptVerifySignature(
                key,
                Some((&padding as *const BCRYPT_PKCS1_PADDING_INFO).cast()),
                hash,
                signature,
                BCRYPT_PAD_PKCS1,
            )
            .ok()
        } else {
            match ecdsa_raw(signature) {
                Ok(sig) => BCryptVerifySignature(key, None, hash, &sig, BCRYPT_FLAGS(0)).ok(),
                Err(e) => {
                    let _ = BCryptDestroyKey(key);
                    return Err(e);
                }
            }
        };
        let _ = BCryptDestroyKey(key);
        result.map_err(|_| "Tanda tangan paket CRX tidak valid".into())
    }
}
fn safe_path(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && name.trim_end_matches('/').split('/').all(|part| {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "<>:\"|?*".contains(c))
                && !matches!(
                    stem.as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
}
fn extract_zip(bytes: &[u8], folder: &Path) -> Result<()> {
    let end = (bytes.len().saturating_sub(65557)..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            bytes.get(i..i + 4) == Some(b"PK\x05\x06")
                && u16_at(bytes, i + 20).is_ok_and(|n| i + 22 + n as usize == bytes.len())
        })
        .ok_or("ZIP tidak valid")?;
    let count = u16_at(bytes, end + 10)? as usize;
    let mut offset = u32_at(bytes, end + 16)? as usize;
    let central = offset;
    let central_end = central
        .checked_add(u32_at(bytes, end + 12)? as usize)
        .ok_or("ZIP overflow")?;
    if u16_at(bytes, end + 4)? != 0
        || u16_at(bytes, end + 6)? != 0
        || u16_at(bytes, end + 8)? as usize != count
        || count > 10000
        || central_end > end
    {
        return Err("ZIP multi-disk/ZIP64 atau terlalu besar tidak didukung".into());
    }
    let mut total = 0u64;
    let mut names = HashSet::new();
    for _ in 0..count {
        if bytes.get(offset..offset + 4) != Some(b"PK\x01\x02") {
            return Err("Direktori ZIP tidak valid".into());
        }
        let flags = u16_at(bytes, offset + 8)?;
        let method = u16_at(bytes, offset + 10)?;
        let crc = u32_at(bytes, offset + 16)?;
        let compressed = u32_at(bytes, offset + 20)? as usize;
        let size = u32_at(bytes, offset + 24)? as usize;
        let n = u16_at(bytes, offset + 28)? as usize;
        let extra = u16_at(bytes, offset + 30)? as usize;
        let comment = u16_at(bytes, offset + 32)? as usize;
        let local = u32_at(bytes, offset + 42)? as usize;
        let name = std::str::from_utf8(
            bytes
                .get(offset + 46..offset + 46 + n)
                .ok_or("Nama ZIP terpotong")?,
        )?;
        if !safe_path(name)
            || !names.insert(name.to_lowercase())
            || flags & 1 != 0
            || !matches!(method, 0 | 8)
            || (u32_at(bytes, offset + 38)? >> 16) & 0xf000 == 0xa000
            || u16_at(bytes, offset + 34)? != 0
        {
            return Err("Path, symlink, duplikat, atau metode ZIP tidak didukung".into());
        }
        total += size as u64;
        if size > 64 * 1024 * 1024 || total > 500 * 1024 * 1024 {
            return Err("Hasil ekstraksi terlalu besar".into());
        }
        let path = folder.join(name);
        if name.ends_with('/') {
            fs::create_dir_all(&path)?;
        } else {
            if bytes.get(local..local + 4) != Some(b"PK\x03\x04")
                || u16_at(bytes, local + 8)? != method
            {
                return Err("Header berkas ZIP tidak valid".into());
            }
            let ln = u16_at(bytes, local + 26)? as usize;
            let le = u16_at(bytes, local + 28)? as usize;
            let start = local.checked_add(30 + ln + le).ok_or("ZIP overflow")?;
            let stop = start.checked_add(compressed).ok_or("ZIP overflow")?;
            if stop > central || bytes.get(local + 30..local + 30 + ln) != Some(name.as_bytes()) {
                return Err("Data ZIP tidak valid".into());
            }
            let data = bytes.get(start..stop).ok_or("Data ZIP terpotong")?;
            let mut output = Vec::new();
            if method == 0 {
                output.extend_from_slice(data)
            } else {
                flate2::read::DeflateDecoder::new(data)
                    .take(size as u64 + 1)
                    .read_to_end(&mut output)?;
            }
            if output.len() != size || crc32fast::hash(&output) != crc {
                return Err("Ukuran atau checksum ZIP tidak cocok".into());
            }
            fs::create_dir_all(path.parent().ok_or("Path tidak valid")?)?;
            use std::io::Write;
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path)?
                .write_all(&output)?;
        }
        offset = offset
            .checked_add(46 + n + extra + comment)
            .ok_or("ZIP overflow")?;
        if offset > central_end {
            return Err("Direktori ZIP terpotong".into());
        }
    }
    if offset != central_end {
        return Err("Ukuran direktori ZIP tidak cocok".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricts_store_urls_and_windows_paths() {
        let id = "aapbdbdomjkkjkaonfhkkikfgjllcleb";
        assert_eq!(store_id(id), Some(id.into()));
        assert_eq!(
            store_id(&format!(
                "https://chromewebstore.google.com/detail/google-translate/{id}"
            )),
            Some(id.into())
        );
        for url in [
            format!("http://chromewebstore.google.com/detail/x/{id}"),
            format!("https://evil.example/detail/x/{id}"),
            format!("https://chromewebstore.google.com.evil.example/detail/x/{id}"),
        ] {
            assert!(store_id(&url).is_none())
        }
        for p in [
            "../a", "/a", "C:/a", "a\\b", "NUL.txt", "a/../b", "a.", "a:stream", "a//b",
        ] {
            assert!(!safe_path(p), "{p}")
        }
        assert!(safe_path("_locales/en/messages.json"));
    }
    #[test]
    fn rejects_unsigned_and_truncated_crx() {
        for bytes in [b"Cr24".as_slice(), b"not a package".as_slice()] {
            assert!(verify_crx(bytes, "aapbdbdomjkkjkaonfhkkikfgjllcleb").is_err())
        }
    }
    #[test]
    fn verifies_official_fixture_when_present() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache/webstore-translate.crx");
        if !path.exists() {
            return;
        }
        let mut bytes = fs::read(path).unwrap();
        let id = "aapbdbdomjkkjkaonfhkkikfgjllcleb";
        assert!(verify_crx(&bytes, id).is_ok());
        assert!(verify_crx(&bytes, "abcdefghijklmnopabcdefghijklmnop").is_err());
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(verify_crx(&bytes, id).is_err());
    }
}
