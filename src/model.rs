use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub title: String,
    pub url: String,
    pub time: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Download {
    #[serde(default)]
    pub id: u64,
    pub url: String,
    pub path: String,
    pub status: String,
    pub time: u64,
    #[serde(default)]
    pub received: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(default = "download_already_seen")]
    pub seen: bool,
}
fn download_already_seen() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dark: bool,
    pub search: String,
    pub restore: bool,
    pub show_bookmarks: bool,
    pub memory_saver: bool,
    pub adblock_enabled: bool,
    pub adblock_allowed_sites: Vec<String>,
    pub download_ask: bool,
    pub download_directory: String,
    pub video_fullscreen: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            dark: false,
            search: "google".into(),
            restore: true,
            show_bookmarks: false,
            memory_saver: true,
            adblock_enabled: true,
            adblock_allowed_sites: Vec::new(),
            download_ask: true,
            download_directory: String::new(),
            video_fullscreen: true,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub extensions: std::collections::BTreeMap<String, ExtensionSource>,
    pub extension_pins: std::collections::BTreeMap<String, bool>,
    pub home: Home,
    pub bookmarks: Vec<Entry>,
    pub history: Vec<Entry>,
    pub downloads: Vec<Download>,
    pub settings: Settings,
    pub session: Vec<String>,
    pub session_pins: Vec<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtensionSource {
    pub store_id: Option<String>,
    pub folder: String,
    pub version: String,
    pub page: Option<String>,
    pub popup: Option<String>,
    pub action_title: String,
    pub icon: String,
    pub permissions: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Shortcut {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Home {
    pub shortcuts: Vec<Shortcut>,
    pub icons: std::collections::BTreeMap<String, String>,
    pub background_color: String,
    pub background_image: bool,
}
impl Default for Home {
    fn default() -> Self {
        Self {
            shortcuts: [
                ("Google", "https://www.google.com/"),
                ("YouTube", "https://www.youtube.com/"),
                ("Gmail", "https://mail.google.com/"),
                ("WhatsApp", "https://web.whatsapp.com/"),
                ("GitHub", "https://github.com/"),
            ]
            .into_iter()
            .map(|(title, url)| Shortcut {
                title: title.into(),
                url: url.into(),
            })
            .collect(),
            icons: Default::default(),
            background_color: String::new(),
            background_image: false,
        }
    }
}

pub fn website_origin(value: &str) -> Option<String> {
    url::Url::parse(value)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .map(|u| u.origin().ascii_serialization())
}

pub fn shortcut_url(input: &str) -> Option<String> {
    let value = input.trim();
    if value.len() > 4096 || value.chars().any(char::is_whitespace) {
        return None;
    }
    let parsed = url::Url::parse(value)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .or_else(|| {
            if value.contains("://")
                || !(value.contains('.')
                    || value.starts_with("localhost")
                    || value.starts_with("[::1]"))
            {
                return None;
            }
            let scheme = if value.starts_with("localhost")
                || value.starts_with("127.")
                || value.starts_with("[::1]")
            {
                "http"
            } else {
                "https"
            };
            url::Url::parse(&format!("{scheme}://{value}")).ok()
        })?;
    (matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some())
        .then(|| parsed.into())
}
impl Store {
    pub fn read(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|s| serde_json::from_slice(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temporary, path)
    }
}

pub fn address(input: &str, search: &str) -> String {
    let s = input.trim();
    if s.is_empty() || s == "azy://newtab" || s == "about:blank" {
        return "azy://newtab".into();
    }
    // Windows Explorer's "Copy as path" includes surrounding quotation marks.
    let path = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s);
    if std::path::Path::new(path).is_absolute() {
        if let Ok(url) = url::Url::from_file_path(path) {
            return url.into();
        }
    }
    if let Ok(u) = url::Url::parse(s) {
        if matches!(u.scheme(), "http" | "https" | "file")
            || (u.scheme() == "chrome-extension"
                && u.host_str().is_some_and(|id| {
                    id.len() == 32 && id.bytes().all(|c| (b'a'..=b'p').contains(&c))
                }))
        {
            return u.into();
        }
    }
    if !s.chars().any(char::is_whitespace)
        && !s.contains("://")
        && (s.contains('.') || s == "localhost" || s.starts_with("localhost:"))
    {
        let scheme =
            if s.starts_with("localhost") || s.starts_with("127.") || s.starts_with("[::1]") {
                "http"
            } else {
                "https"
            };
        if let Ok(u) = url::Url::parse(&format!("{scheme}://{s}")) {
            return u.into();
        }
    }
    let base = if search == "duckduckgo" {
        "https://duckduckgo.com/"
    } else if search == "bing" {
        "https://www.bing.com/search"
    } else {
        "https://www.google.com/search"
    };
    let mut u = url::Url::parse(base).unwrap();
    u.query_pairs_mut().append_pair("q", s);
    u.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addresses_and_searches() {
        assert_eq!(address("example.com/a", "google"), "https://example.com/a");
        assert_eq!(
            address("localhost:8080", "google"),
            "http://localhost:8080/"
        );
        assert_eq!(address("  ", "google"), "azy://newtab");
        assert!(
            address("javascript:alert(1)", "google").starts_with("https://www.google.com/search?")
        );
        assert_eq!(
            url::Url::parse(&address("a & b", "bing"))
                .unwrap()
                .query_pairs()
                .next()
                .unwrap()
                .1,
            "a & b"
        );
    }
    #[test]
    #[cfg(windows)]
    fn local_paths_preserve_filename_characters() {
        for input in [
            r#"C:\Dokumen Saya\laporan #1 ü.pdf"#,
            r#""C:\Dokumen Saya\laporan #1 ü.pdf""#,
        ] {
            let url = url::Url::parse(&address(input, "google")).unwrap();
            assert_eq!(url.scheme(), "file");
            assert_eq!(
                url.to_file_path().unwrap(),
                std::path::Path::new(r#"C:\Dokumen Saya\laporan #1 ü.pdf"#)
            );
            assert!(url.fragment().is_none());
        }
        assert_eq!(
            address(r"\\server\share\report.pdf", "google"),
            "file://server/share/report.pdf"
        );
    }
    #[test]
    fn home_migrates_old_profiles_and_preserves_an_empty_shortcut_list() {
        let legacy: Store =
            serde_json::from_str(r#"{"bookmarks":[],"settings":{"dark":true}}"#).unwrap();
        assert_eq!(legacy.home.shortcuts.len(), 5);
        assert!(legacy.extensions.is_empty());
        assert!(legacy.settings.dark);
        assert!(legacy.settings.video_fullscreen);
        assert!(legacy.settings.download_ask);
        assert!(legacy.settings.download_directory.is_empty());
        let empty: Store = serde_json::from_str(r#"{"home":{"shortcuts":[]}}"#).unwrap();
        assert!(empty.home.shortcuts.is_empty());
        assert!(!empty.home.background_image);
        assert_eq!(
            shortcut_url("example.com/path?a=1&b=2"),
            Some("https://example.com/path?a=1&b=2".into())
        );
        assert_eq!(
            shortcut_url("localhost:8080/app"),
            Some("http://localhost:8080/app".into())
        );
        for invalid in [
            "javascript:alert(1)",
            "file:///C:/secret",
            "data:text/html,hello",
            "a search query",
            "https://",
        ] {
            assert!(shortcut_url(invalid).is_none(), "{invalid}");
        }
    }
    #[test]
    fn downloads_migrate_progress_and_notification_state() {
        let download: Download = serde_json::from_str(
            r#"{"url":"https://example.com/a","path":"a.txt","status":"complete","time":1}"#,
        )
        .unwrap();
        assert_eq!(download.received, 0);
        assert_eq!(download.total, 0);
        assert!(download.seen);
        let settings: Settings = serde_json::from_str(r#"{"video_fullscreen":false}"#).unwrap();
        assert!(!settings.video_fullscreen);
    }
    #[test]
    fn persists_without_losing_unicode() {
        let path = std::env::temp_dir().join(format!("azy-store-{}.json", std::process::id()));
        let mut s = Store::default();
        s.bookmarks.push(Entry {
            title: "Azy • Indonesia".into(),
            url: "https://example.com".into(),
            time: 1,
        });
        s.save(&path).unwrap();
        s.save(&path).unwrap();
        assert_eq!(Store::read(&path).bookmarks[0].title, "Azy • Indonesia");
        fs::remove_file(path).unwrap();
    }
}
