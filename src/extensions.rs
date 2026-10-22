use crate::{model::ExtensionSource, Msg};
use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, fs, path::Path, rc::Rc};
use tao::event_loop::EventLoopProxy;
use webview2_com::{
    BrowserExtensionEnableCompletedHandler, BrowserExtensionRemoveCompletedHandler,
    Microsoft::Web::WebView2::Win32::*, ProfileAddBrowserExtensionCompletedHandler,
    ProfileGetBrowserExtensionsCompletedHandler,
};
use windows::core::{Interface, BOOL, PCWSTR, PWSTR};
use wry::{WebView, WebViewExtWindows};

#[derive(Clone, Debug, Serialize)]
pub struct Info {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

pub struct Manager {
    profile: ICoreWebView2Profile7,
    handles: Rc<RefCell<HashMap<String, ICoreWebView2BrowserExtension>>>,
}
impl Manager {
    pub fn new(view: &WebView) -> windows::core::Result<Self> {
        let profile = unsafe {
            view.webview()
                .cast::<ICoreWebView2_13>()?
                .Profile()?
                .cast()?
        };
        Ok(Self {
            profile,
            handles: Default::default(),
        })
    }
    pub fn refresh(&self, proxy: EventLoopProxy<Msg>) -> windows::core::Result<()> {
        let handles = self.handles.clone();
        let callback =
            ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |status, list| {
                if let Err(error) = status {
                    let _ = proxy.send_event(Msg::ExtensionError(error.to_string()));
                    return Ok(());
                }
                let mut rows = Vec::new();
                let mut next = HashMap::new();
                if let Some(list) = list {
                    unsafe {
                        let mut count = 0;
                        list.Count(&mut count)?;
                        for index in 0..count {
                            let extension = list.GetValueAtIndex(index)?;
                            let mut id = PWSTR::null();
                            let mut name = PWSTR::null();
                            let mut enabled = BOOL(0);
                            extension.Id(&mut id)?;
                            let id = webview2_com::take_pwstr(id);
                            // Runtime components are not user-installed browser extensions.
                            if matches!(
                                id.as_str(),
                                "dgiklkfkllikcanfonkcabmbdfmgleag"
                                    | "mhjfbmdgcfjbbpaeojofohoefgiehjai"
                            ) {
                                continue;
                            }
                            extension.Name(&mut name)?;
                            let name = webview2_com::take_pwstr(name);
                            extension.IsEnabled(&mut enabled)?;
                            rows.push(Info {
                                id: id.clone(),
                                name,
                                enabled: enabled.as_bool(),
                            });
                            next.insert(id, extension);
                        }
                    }
                }
                rows.sort_by(|a, b| a.name.cmp(&b.name));
                *handles.borrow_mut() = next;
                let _ = proxy.send_event(Msg::ExtensionList(rows));
                Ok(())
            }));
        unsafe { self.profile.GetBrowserExtensions(&callback) }
    }
    pub fn install(
        &self,
        folder: &str,
        proxy: EventLoopProxy<Msg>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = read_source(Path::new(folder))?;
        self.install_source(source, proxy)
    }
    pub fn install_source(
        &self,
        source: ExtensionSource,
        proxy: EventLoopProxy<Msg>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let wide: Vec<u16> = source.folder.encode_utf16().chain(Some(0)).collect();
        let callback = ProfileAddBrowserExtensionCompletedHandler::create(Box::new(
            move |status, extension| {
                match (status, extension) {
                    (Ok(()), Some(extension)) => {
                        let mut id = PWSTR::null();
                        unsafe {
                            extension.Id(&mut id)?;
                        }
                        let id = webview2_com::take_pwstr(id);
                        let _ = proxy.send_event(Msg::ExtensionInstalled(id, source.clone()));
                    }
                    (result, _) => {
                        let _ = proxy.send_event(Msg::ExtensionError(format!("Runtime menolak ekstensi. Periksa manifest dan kompatibilitas ekstensi. {result:?}")));
                    }
                }
                Ok(())
            },
        ));
        unsafe {
            self.profile
                .AddBrowserExtension(PCWSTR(wide.as_ptr()), &callback)?;
        }
        Ok(())
    }
    pub fn enable(
        &self,
        id: &str,
        enabled: bool,
        proxy: EventLoopProxy<Msg>,
    ) -> windows::core::Result<()> {
        let extension = self.handles.borrow().get(id).cloned().ok_or_else(|| {
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80070002u32 as i32))
        })?;
        let callback = BrowserExtensionEnableCompletedHandler::create(Box::new(move |status| {
            let msg = match status {
                Ok(()) => Msg::ExtensionDone(None),
                Err(error) => Msg::ExtensionError(error.to_string()),
            };
            let _ = proxy.send_event(msg);
            Ok(())
        }));
        unsafe { extension.Enable(enabled, &callback) }
    }
    pub fn remove(&self, id: &str, proxy: EventLoopProxy<Msg>) -> windows::core::Result<()> {
        let extension = self.handles.borrow().get(id).cloned().ok_or_else(|| {
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80070002u32 as i32))
        })?;
        let id = id.to_string();
        let callback = BrowserExtensionRemoveCompletedHandler::create(Box::new(move |status| {
            let msg = match status {
                Ok(()) => Msg::ExtensionDone(Some(id.clone())),
                Err(error) => Msg::ExtensionError(error.to_string()),
            };
            let _ = proxy.send_event(msg);
            Ok(())
        }));
        unsafe { extension.Remove(&callback) }
    }
}

pub fn read_source(folder: &Path) -> Result<ExtensionSource, Box<dyn std::error::Error>> {
    let folder = folder.canonicalize()?;
    let file = folder.join("manifest.json");
    if fs::metadata(&file)?.len() > 1024 * 1024 {
        return Err("manifest.json terlalu besar".into());
    }
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(file)?)?;
    if !matches!(manifest["manifest_version"].as_u64(), Some(2 | 3))
        || manifest["name"].as_str().is_none()
        || manifest["version"].as_str().is_none()
    {
        return Err("Manifest ekstensi tidak valid".into());
    }
    let page = manifest["options_ui"]["page"]
        .as_str()
        .or(manifest["options_page"].as_str());
    let permissions = ["permissions", "host_permissions"]
        .into_iter()
        .flat_map(|key| manifest[key].as_array().into_iter().flatten())
        .filter_map(|v| v.as_str().map(String::from))
        .take(100)
        .collect();
    let action = manifest
        .get("action")
        .or_else(|| manifest.get("browser_action"))
        .or_else(|| manifest.get("page_action"));
    let popup = action
        .and_then(|a| a["default_popup"].as_str())
        .filter(|p| !p.is_empty())
        .map(String::from);
    let icons = action
        .and_then(|a| a.get("default_icon"))
        .unwrap_or(&manifest["icons"]);
    let icon_path = icons.as_str().or_else(|| {
        ["32", "48", "16", "128"]
            .into_iter()
            .find_map(|size| icons[size].as_str())
    });
    let icon = icon_path
        .and_then(|p| read_icon(&folder, p))
        .unwrap_or_default();
    Ok(ExtensionSource {
        store_id: None,
        folder: folder.to_string_lossy().into(),
        version: manifest["version"].as_str().unwrap().into(),
        page: page.map(String::from),
        permissions,
        popup,
        action_title: action
            .and_then(|a| a["default_title"].as_str())
            .unwrap_or("")
            .into(),
        icon,
    })
}
fn read_icon(folder: &Path, name: &str) -> Option<String> {
    let file = folder.join(name).canonicalize().ok()?;
    if !file.starts_with(folder) || fs::metadata(&file).ok()?.len() > 1024 * 1024 {
        return None;
    }
    let bytes = fs::read(file).ok()?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let icon = reader.decode().ok()?.thumbnail(32, 32);
    let mut output = std::io::Cursor::new(Vec::new());
    icon.write_to(&mut output, image::ImageFormat::Png).ok()?;
    Some(
        output
            .into_inner()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

// Each WebView is a separate Chromium window. Only this popup's current-window
// queries are redirected; native permission checks and real tab IDs are retained.
pub fn popup_script(id: &str, window_id: i64) -> String {
    format!(
        r#"(()=>{{
      if(location.protocol!=='chrome-extension:'||location.hostname!=={id})return;
      const tabs=chrome.tabs,query=tabs.query.bind(tabs),target={window_id};
      tabs.query=function(info,callback){{
        const q={{...info}};
        if(q.currentWindow===true||q.lastFocusedWindow===true||(q.active===true&&q.windowId==null&&q.currentWindow!==false)){{delete q.currentWindow;delete q.lastFocusedWindow;q.windowId=target;}}
        return typeof callback==='function'?query(q,callback):query(q);
      }};
      tabs.getCurrent=function(callback){{const result=query({{windowId:target,active:true}}).then(t=>t[0]);if(typeof callback==='function'){{result.then(callback);return;}}return result;}};
      if(chrome.windows){{const get=chrome.windows.get.bind(chrome.windows);chrome.windows.getCurrent=function(info,callback){{if(typeof info==='function')return get(target,info);return typeof callback==='function'?get(target,info||{{}},callback):get(target,info||{{}});}};}}
    }})();"#,
        id = serde_json::to_string(id).unwrap()
    )
}
pub fn page_url(id: &str, page: &str) -> Option<String> {
    if id.len() != 32 || !id.bytes().all(|c| (b'a'..=b'p').contains(&c)) {
        return None;
    }
    let root = url::Url::parse(&format!("chrome-extension://{id}/")).ok()?;
    let target = root.join(page).ok()?;
    (target.scheme() == "chrome-extension" && target.host_str() == Some(id)).then(|| target.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_are_restricted_to_the_installed_extension_origin() {
        let id = "abcdefghijklmnopabcdefghijklmnop";
        assert_eq!(
            page_url(id, "settings/options.html"),
            Some(format!("chrome-extension://{id}/settings/options.html"))
        );
        for page in [
            "https://example.com/",
            "javascript:alert(1)",
            "//bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/options.html",
        ] {
            assert!(page_url(id, page).is_none());
        }
        assert!(page_url("invalid-id", "options.html").is_none());
    }
}
