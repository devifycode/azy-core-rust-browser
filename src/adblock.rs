use std::{cell::RefCell, collections::HashSet, rc::Rc};
use url::Url;
use webview2_com::{Microsoft::Web::WebView2::Win32::*, WebResourceRequestedEventHandler};
use windows::{
    core::{w, Interface, PWSTR},
    Win32::System::Com::CoTaskMemFree,
};
use wry::{WebView, WebViewExtWindows};

pub struct Blocker {
    domains: HashSet<&'static str>,
    pub enabled: bool,
    pub allowed_sites: HashSet<String>,
}
#[derive(Default)]
pub struct Page {
    pub host: String,
    pub blocked: u64,
}
impl Page {
    pub fn navigate(&mut self, url: &str) {
        self.host = site_host(url).unwrap_or_default();
        self.blocked = 0;
    }
}
pub fn site_host(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.host_str()
        .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
}
impl Blocker {
    pub fn new(enabled: bool, allowed: &[String]) -> Self {
        Self {
            domains: include_str!("../assets/filters/domains.txt")
                .lines()
                .filter(|s| !s.starts_with('#') && !s.is_empty())
                .collect(),
            enabled,
            allowed_sites: allowed.iter().cloned().collect(),
        }
    }
    pub fn rule_count(&self) -> usize {
        self.domains.len()
    }
    pub fn active_for(&self, url: &str) -> bool {
        self.enabled && site_host(url).is_some_and(|host| !self.allowed_sites.contains(&host))
    }
    pub fn should_block(&self, request: &Url, page: &Page, document: bool) -> bool {
        if !self.enabled
            || self.allowed_sites.contains(&page.host)
            || !matches!(request.scheme(), "http" | "https")
        {
            return false;
        }
        let Some(host) = request.host_str() else {
            return false;
        };
        let host = host.trim_end_matches('.');
        // Explicit top-level navigation is always permitted, including redirects.
        if document && host == page.host {
            return false;
        }
        let mut candidate = host;
        loop {
            if self.domains.contains(candidate) {
                return true;
            }
            match candidate.split_once('.') {
                Some((_, parent)) => candidate = parent,
                None => return false,
            }
        }
    }
}

pub fn attach(
    view: &WebView,
    blocker: Rc<RefCell<Blocker>>,
    page: Rc<RefCell<Page>>,
) -> wry::Result<()> {
    let environment = view.environment();
    let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        // Fail open on inspection/response errors; do not interrupt unrelated content.
        let result: windows::core::Result<()> = unsafe {
            (|| {
                let request = args.Request()?;
                let mut raw = PWSTR::null();
                request.Uri(&mut raw)?;
                let uri = raw.to_string();
                CoTaskMemFree(Some(raw.0.cast()));
                let Ok(uri) = uri else {
                    return Ok(());
                };
                let Ok(url) = Url::parse(&uri) else {
                    return Ok(());
                };
                let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
                args.ResourceContext(&mut context)?;
                let blocked = blocker.borrow().should_block(
                    &url,
                    &page.borrow(),
                    context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
                );
                if blocked {
                    let response = environment.CreateWebResourceResponse(
                        None,
                        204,
                        w!("No Content"),
                        w!("Content-Length: 0\r\nCache-Control: no-store\r\n"),
                    )?;
                    args.SetResponse(&response)?;
                    let mut state = page.borrow_mut();
                    state.blocked = state.blocked.saturating_add(1);
                }
                Ok(())
            })()
        };
        let _ = result;
        Ok(())
    }));
    unsafe {
        let native = view.webview();
        let mut token = 0;
        native.add_WebResourceRequested(&handler, &mut token)?;
        for pattern in [w!("http://*"), w!("https://*")] {
            if let Ok(modern) = native.cast::<ICoreWebView2_22>() {
                modern.AddWebResourceRequestedFilterWithRequestSourceKinds(
                    pattern,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                )?;
            } else {
                native.AddWebResourceRequestedFilter(
                    pattern,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_only_domain_boundaries_and_never_top_level_navigation() {
        let b = Blocker::new(true, &[]);
        let mut p = Page::default();
        p.navigate("https://example.com/");
        for u in [
            "https://doubleclick.net/a.js",
            "https://ads.doubleclick.net/a.js",
            "https://ads.doubleclick.net./a",
        ] {
            assert!(b.should_block(&Url::parse(u).unwrap(), &p, false), "{u}");
        }
        for u in [
            "https://notdoubleclick.net/a",
            "https://doubleclick.net.example.org/a",
            "https://example.com/?next=doubleclick.net",
        ] {
            assert!(!b.should_block(&Url::parse(u).unwrap(), &p, false), "{u}");
        }
        p.navigate("https://ads.doubleclick.net/landing");
        assert!(!b.should_block(
            &Url::parse("https://ads.doubleclick.net/landing").unwrap(),
            &p,
            true
        ));
        assert!(b.rule_count() > 10000);
    }
    #[test]
    fn exceptions_and_disable_apply_without_rebuilding_rules() {
        let mut b = Blocker::new(true, &["example.com".into()]);
        let mut p = Page::default();
        p.navigate("https://example.com/");
        let ad = Url::parse("https://ads.doubleclick.net/a.js").unwrap();
        assert!(!b.should_block(&ad, &p, false));
        p.navigate("https://other.example/");
        assert!(b.should_block(&ad, &p, false));
        b.enabled = false;
        assert!(!b.should_block(&ad, &p, false));
    }
}
