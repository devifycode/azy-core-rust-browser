#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod adblock;
mod downloads;
mod extensions;
mod favicon;
mod instance;
mod model;
mod updater;
mod webstore;
use model::{address, now, Download, Entry, Store};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::{Fullscreen, Icon, Window, WindowBuilder},
};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, CallDevToolsProtocolMethodCompletedHandler,
    ContainsFullScreenElementChangedEventHandler, DOMContentLoadedEventHandler,
    FaviconChangedEventHandler, FocusChangedEventHandler, GetFaviconCompletedHandler,
    HistoryChangedEventHandler, Microsoft::Web::WebView2::Win32::*,
    NavigationCompletedEventHandler, NavigationStartingEventHandler, SourceChangedEventHandler,
    TrySuspendCompletedHandler, WindowCloseRequestedEventHandler, ZoomFactorChangedEventHandler,
};
use windows::{
    core::{Interface, BOOL},
    Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
};
use wry::{
    dpi::{LogicalPosition as WPosition, LogicalSize as WSize},
    NewWindowResponse, Rect, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows,
    WebViewExtWindows,
};

fn diagnostic(message: &str) {
    if let Some(root) = std::env::var_os("AZY_CORE_DATA_DIR") {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(PathBuf::from(root).join("diagnostic.log"))
        {
            let _ = writeln!(f, "{} {message}", now());
        }
    }
}
#[derive(Debug, Clone)]
enum Msg {
    ContentFullscreen(u64, bool),
    DownloadProgress(u64, u64, u64),
    UpdateChecked(Result<Option<updater::Available>, String>),
    UpdateDownloaded(Result<PathBuf, String>),
    UpdateProgress(u8),
    ExternalOpen(Vec<String>, bool),
    StorePrepared(Result<webstore::Prepared, String>),
    ExtensionPopupReady(u64, u64, String, i64),
    ExtensionPopupClose(u64),
    ExtensionPopupDismiss,
    ContentFocused(u64),
    ExtensionList(Vec<extensions::Info>),
    ExtensionInstalled(String, model::ExtensionSource),
    ExtensionDone(Option<String>),
    ExtensionError(String),
    Ui(String),
    New(String, bool),
    Title(u64, String),
    Load(u64, String, bool),
    NavigationStarted(u64, u64, String),
    NavigationFinished(u64, u64),
    ContentReady(u64, u64),
    Refresh(u64),
    FaviconChanged(u64),
    Favicon(u64, String, String),
    ZoomChanged(u64, f64),
    Suspended(u64, bool),
    DownloadRequested(u64),
    DownloadFinished(u64, bool),
    Key(String),
}
struct Tab {
    id: u64,
    title: String,
    url: String,
    private: bool,
    pinned: bool,
    loading: bool,
    view: Option<WebView>,
    zoom: f64,
    favicon: String,
    visible: bool,
    presented: bool,
    reveal_at: Option<Instant>,
    navigation_id: Option<u64>,
    suspended: bool,
    suspend_pending: bool,
    hidden_since: Instant,
    last_bounds: Option<(f64, f64, f64, f64)>,
    network: Rc<RefCell<adblock::Page>>,
}
struct Browser {
    content_fullscreen: Option<(u64, bool)>,
    update: updater::State,
    update_available: Option<updater::Available>,
    update_plan: Option<PathBuf>,
    next_update_check: Instant,
    window: Window,
    ui: WebView,
    tabs: Vec<Tab>,
    active: u64,
    next: u64,
    split: Option<[u64; 2]>,
    split_ratio: f64,
    context: WebContext,
    private_context: WebContext,
    extension_host: Option<WebView>,
    extension_manager: Option<extensions::Manager>,
    extension_list: Vec<extensions::Info>,
    extension_busy: bool,
    extension_ready: bool,
    extension_error: String,
    store_prepared: Option<webstore::Prepared>,
    extension_popup: Option<WebView>,
    extension_popup_id: String,
    extension_popup_generation: u64,
    store: Store,
    root: PathBuf,
    panel: String,
    proxy: EventLoopProxy<Msg>,
    closed: Vec<(String, bool, bool)>,
    exit: bool,
    last_saved: Option<Vec<u8>>,
    last_ui_state: RefCell<Value>,
    next_maintenance: Instant,
    ui_bounds: Option<(f64, f64)>,
    ui_overlay: Option<(f64, f64, f64, f64)>,
    ui_overlay_radius: f64,
    ui_region: Option<String>,
    blocker: Rc<RefCell<adblock::Blocker>>,
    downloads: downloads::Manager,
}

impl Tab {
    fn set_url(&mut self, url: String) {
        let origin = |s: &str| {
            url::Url::parse(s)
                .ok()
                .map(|u| u.origin().ascii_serialization())
        };
        if origin(&self.url) != origin(&url) || url == "azy://newtab" {
            self.favicon.clear();
        }
        self.url = url;
    }
}

fn bounds(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        position: WPosition::new(x, y).into(),
        size: WSize::new(width.max(1.), height.max(1.)).into(),
    }
}

fn browser_background(dark: bool) -> (u8, u8, u8, u8) {
    if dark {
        (32, 33, 36, 255)
    } else {
        (255, 255, 255, 255)
    }
}

// Browser shortcuts are handled by the native controller, never page JavaScript.
fn shortcuts(view: &WebView, proxy: EventLoopProxy<Msg>) {
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                    && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                {
                    return Ok(());
                }
                let mut key = 0;
                args.VirtualKey(&mut key)?;
                let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
                let alt = GetKeyState(VK_MENU.0 as i32) < 0;
                let command = match (ctrl, shift, alt, key) {
                    (true, true, _, 78) => "private",
                    (true, true, _, 84) => "reopen",
                    (true, true, _, 66) => "toggle-bookmarks",
                    (true, true, _, 9) => "previous",
                    (true, _, _, 9) => "next",
                    (true, _, _, 76) | (_, _, true, 68) => "address",
                    (true, _, _, 84) => "new",
                    (true, _, _, 87) => "close-tab",
                    (true, _, _, 78) => "window",
                    (true, _, _, 82) | (_, _, _, 116) => "reload",
                    (true, _, _, 68) => "bookmark",
                    (true, _, _, 72) => "history",
                    (true, _, _, 74) => "downloads",
                    (true, _, _, 70) => "find",
                    (true, _, _, 80) => "print",
                    (true, _, _, 79) => "open-file",
                    (true, _, _, 187) | (true, _, _, 107) => "zoom-in",
                    (true, _, _, 189) | (true, _, _, 109) => "zoom-out",
                    (true, _, _, 48) => "zoom-reset",
                    (_, _, true, 37) => "back",
                    (_, _, true, 39) => "forward",
                    (_, _, _, 122) => "fullscreen",
                    (_, _, _, 27) => "escape",
                    _ => "",
                };
                if !command.is_empty() {
                    args.SetHandled(true)?;
                    let _ = proxy.send_event(Msg::Key(command.into()));
                }
            }
        }
        Ok(())
    }));
    unsafe {
        let mut token = 0;
        let _ = view
            .controller()
            .add_AcceleratorKeyPressed(&handler, &mut token);
    }
}
fn dismiss_popup_on_focus(view: &WebView, proxy: EventLoopProxy<Msg>, id: u64) {
    let focus = FocusChangedEventHandler::create(Box::new(move |_, _| {
        let _ = proxy.send_event(Msg::ExtensionPopupDismiss);
        let _ = proxy.send_event(Msg::ContentFocused(id));
        Ok(())
    }));
    unsafe {
        let mut token = 0;
        let _ = view.controller().add_GotFocus(&focus, &mut token);
    }
}

impl Browser {
    fn top(&self) -> f64 {
        if self.store.settings.show_bookmarks {
            120.
        } else {
            86.
        }
    }
    fn current(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active)
    }
    fn create_view(
        &mut self,
        id: u64,
        url: &str,
        private: bool,
        network: Rc<RefCell<adblock::Page>>,
    ) -> wry::Result<WebView> {
        self.create_view_with_popup(id, url, private, network, None)
    }
    fn create_view_with_popup(
        &mut self,
        id: u64,
        url: &str,
        private: bool,
        network: Rc<RefCell<adblock::Page>>,
        popup: Option<(&str, i64)>,
    ) -> wry::Result<WebView> {
        let p3 = self.proxy.clone();
        let p4 = self.proxy.clone();

        let initial = if url == "azy://newtab" {
            "about:blank"
        } else {
            url
        };
        let top = self.top();
        let size = self
            .window
            .inner_size()
            .to_logical::<f64>(self.window.scale_factor());
        let navigation_state = network.clone();
        let popup_origin = popup.map(|(id, _)| id.to_string());
        let popup_navigation = self.proxy.clone();
        let script = popup
            .map(|(id, window)| extensions::popup_script(id, window))
            .unwrap_or_default();
        let context = if private {
            &mut self.private_context
        } else {
            &mut self.context
        };
        let view = WebViewBuilder::new_with_web_context(context)
            .with_additional_browser_args("")
            .with_browser_extensions_enabled(!private)
            .with_focused(id != 0)
            .with_incognito(private)
            .with_hotkeys_zoom(true)
            .with_visible(false)
            .with_background_color(browser_background(false))
            .with_devtools(true)
            .with_initialization_script(&script)
            .with_bounds(bounds(0., top, size.width, size.height - top))
            .with_navigation_handler(move |u| {
                if let Some(id) = &popup_origin {
                    if u == "about:blank" {
                        return true;
                    }
                    let same = url::Url::parse(&u).is_ok_and(|url| {
                        url.scheme() == "chrome-extension" && url.host_str() == Some(id)
                    });
                    if !same && (u.starts_with("https://") || u.starts_with("http://")) {
                        let _ = popup_navigation.send_event(Msg::New(u, false));
                    }
                    return same;
                }
                let allowed = u == "about:blank"
                    || url::Url::parse(&u).is_ok_and(|u| {
                        matches!(
                            u.scheme(),
                            "https" | "http" | "file" | "blob" | "data" | "chrome-extension"
                        )
                    });
                if allowed {
                    navigation_state.borrow_mut().navigate(&u);
                }
                allowed
            })
            .with_document_title_changed_handler(move |title| {
                let _ = p3.send_event(Msg::Title(id, title));
            })
            .with_new_window_req_handler(move |u, _| {
                let _ = p4.send_event(Msg::New(u, private));
                NewWindowResponse::Deny
            })
            .build_as_child(&self.window)?;
        shortcuts(&view, self.proxy.clone());
        self.downloads
            .attach(&view.webview(), self.proxy.clone())
            .map_err(|e| wry::Error::WebView2Error(e.into()))?;
        if popup.is_none() && id != 0 {
            dismiss_popup_on_focus(&view, self.proxy.clone(), id);
        }
        let p = self.proxy.clone();
        let handler = HistoryChangedEventHandler::create(Box::new(move |_, _| {
            let _ = p.send_event(Msg::Refresh(id));
            Ok(())
        }));
        let p = self.proxy.clone();
        let source = SourceChangedEventHandler::create(Box::new(move |_, _| {
            let _ = p.send_event(Msg::Refresh(id));
            Ok(())
        }));
        let p = self.proxy.clone();
        let zoom_changed = ZoomFactorChangedEventHandler::create(Box::new(move |controller, _| {
            if let Some(controller) = controller {
                let mut factor = 1.;
                unsafe {
                    controller.ZoomFactor(&mut factor)?;
                }
                let _ = p.send_event(Msg::ZoomChanged(id, factor));
            }
            Ok(())
        }));
        unsafe {
            let mut token = 0;
            let proxy = self.proxy.clone();
            let started = NavigationStartingEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut canceled = BOOL(0);
                    args.Cancel(&mut canceled)?;
                    if !canceled.as_bool() {
                        let mut navigation_id = 0;
                        let mut uri = windows::core::PWSTR::null();
                        args.NavigationId(&mut navigation_id)?;
                        args.Uri(&mut uri)?;
                        let _ = proxy.send_event(Msg::NavigationStarted(
                            id,
                            navigation_id,
                            webview2_com::take_pwstr(uri),
                        ));
                    }
                }
                Ok(())
            }));
            view.webview()
                .add_NavigationStarting(&started, &mut token)?;
            let proxy = self.proxy.clone();
            let finished = NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut navigation_id = 0;
                    args.NavigationId(&mut navigation_id)?;
                    let _ = proxy.send_event(Msg::NavigationFinished(id, navigation_id));
                }
                Ok(())
            }));
            view.webview()
                .add_NavigationCompleted(&finished, &mut token)?;
            if let Ok(native) = view.webview().cast::<ICoreWebView2_2>() {
                let proxy = self.proxy.clone();
                let ready = DOMContentLoadedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut navigation_id = 0;
                        args.NavigationId(&mut navigation_id)?;
                        let _ = proxy.send_event(Msg::ContentReady(id, navigation_id));
                    }
                    Ok(())
                }));
                native.add_DOMContentLoaded(&ready, &mut token)?;
            }
            let _ = view
                .controller()
                .add_ZoomFactorChanged(&zoom_changed, &mut token);
            let _ = view.webview().add_HistoryChanged(&handler, &mut token);
            let _ = view.webview().add_SourceChanged(&source, &mut token);
            if let Ok(native) = view.webview().cast::<ICoreWebView2_15>() {
                let p = self.proxy.clone();
                let changed = FaviconChangedEventHandler::create(Box::new(move |_, _| {
                    let _ = p.send_event(Msg::FaviconChanged(id));
                    Ok(())
                }));
                let _ = native.add_FaviconChanged(&changed, &mut token);
            }
        }
        if popup.is_none() && id != 0 {
            let proxy = self.proxy.clone();
            let handler =
                ContainsFullScreenElementChangedEventHandler::create(Box::new(move |view, _| {
                    if let Some(view) = view {
                        let mut full = BOOL(0);
                        unsafe {
                            view.ContainsFullScreenElement(&mut full)?;
                        }
                        let _ = proxy.send_event(Msg::ContentFullscreen(id, full.as_bool()));
                    }
                    Ok(())
                }));
            unsafe {
                let mut token = 0;
                view.webview()
                    .add_ContainsFullScreenElementChanged(&handler, &mut token)?;
            }
        }
        adblock::attach(&view, self.blocker.clone(), network)?;
        view.load_url(initial)?;
        Ok(view)
    }
    fn add_deferred_tab(&mut self, input: &str, private: bool) -> u64 {
        let id = self.next;
        self.next += 1;
        let url = address(input, &self.store.settings.search);
        let title = if url == "azy://newtab" {
            "Tab baru".into()
        } else {
            url::Url::parse(&url)
                .ok()
                .and_then(|u| u.host_str().map(String::from))
                .unwrap_or_else(|| url.clone())
        };
        self.tabs.push(Tab {
            id,
            title,
            url,
            private,
            pinned: false,
            loading: false,
            view: None,
            zoom: 1.,
            favicon: String::new(),
            visible: false,
            presented: false,
            reveal_at: None,
            navigation_id: None,
            suspended: false,
            suspend_pending: false,
            hidden_since: Instant::now(),
            last_bounds: None,
            network: Rc::new(RefCell::new(adblock::Page::default())),
        });
        id
    }
    fn ensure_view(&mut self, id: u64) -> wry::Result<()> {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            if self.tabs[index].view.is_none() && self.tabs[index].url != "azy://newtab" {
                let url = self.tabs[index].url.clone();
                let private = self.tabs[index].private;
                let network = self.tabs[index].network.clone();
                let view = self.create_view(id, &url, private, network)?;
                let _ = view.zoom(self.tabs[index].zoom);
                self.tabs[index].view = Some(view);
                // Keep the chrome's loading surface until the document has parsed.
                // A bounded fallback also reveals error pages, downloads, and slow responses.
                self.tabs[index].presented = false;
                self.tabs[index].reveal_at = Some(Instant::now() + Duration::from_secs(3));
                self.tabs[index].loading = true;
                self.tabs[index].hidden_since = Instant::now();
            }
        }
        Ok(())
    }
    fn activate(&mut self, id: u64) {
        self.leave_content_fullscreen();
        self.close_chrome_overlays();
        self.close_extension_popup();
        self.active = id;
        self.panel.clear();
        if let Err(e) = self.ensure_view(id) {
            self.toast(&e.to_string());
        }
        self.layout();
        self.sync();
    }
    fn new_tab(&mut self, input: &str, private: bool) -> wry::Result<()> {
        self.leave_content_fullscreen();
        self.close_chrome_overlays();
        self.close_extension_popup();
        let id = self.add_deferred_tab(input, private);
        self.active = id;
        self.panel.clear();
        self.ensure_view(id)?;
        self.layout();
        self.sync();
        self.save();
        if self.current().is_some_and(|t| t.url == "azy://newtab") {
            self.focus_address();
        }
        Ok(())
    }
    fn leave_content_fullscreen(&mut self) {
        if let Some((id, was_fullscreen)) = self.content_fullscreen.take() {
            if let Some(view) = self
                .tabs
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.view.as_ref())
            {
                let _ = view.evaluate_script(
                    "if(document.fullscreenElement)document.exitFullscreen().catch(()=>{})",
                );
            }
            self.window
                .set_fullscreen(was_fullscreen.then_some(Fullscreen::Borderless(None)));
            let _ = self.ui.set_visible(true);
            self.ui_region = None;
        }
    }
    fn layout(&mut self) {
        if let Some((id, _)) = self.content_fullscreen {
            let size = self
                .window
                .inner_size()
                .to_logical::<f64>(self.window.scale_factor());
            let _ = self.ui.set_visible(false);
            if let Some(popup) = &self.extension_popup {
                let _ = popup.set_visible(false);
            }
            for tab in &mut self.tabs {
                if let Some(view) = &tab.view {
                    let visible = tab.id == id && !self.window.is_minimized();
                    if tab.visible != visible {
                        let _ = view.set_visible(visible);
                        tab.visible = visible;
                    }
                    if visible {
                        let dimensions = (0., 0., size.width, size.height);
                        if tab.last_bounds != Some(dimensions) {
                            let _ = view.set_bounds(bounds(0., 0., size.width, size.height));
                            tab.last_bounds = Some(dimensions);
                        }
                    }
                }
            }
            return;
        }
        let top = self.top();
        let size = self
            .window
            .inner_size()
            .to_logical::<f64>(self.window.scale_factor());
        let split = self
            .split
            .filter(|ids| ids.contains(&self.active) && self.panel.is_empty());
        let split_x = ((size.width - 8.).max(2.) * self.split_ratio).floor();
        let showing_page = (split.is_some()
            || self
                .current()
                .is_some_and(|t| t.url != "azy://newtab" && t.presented))
            && self.panel.is_empty();
        let overlay = self.ui_overlay;
        let ui_height = if split.is_some() {
            size.height
        } else if showing_page {
            overlay
                .map_or(top, |(_, y, _, height)| top.max(y + height + 6.))
                .min(size.height)
        } else {
            size.height
        };
        if self.ui_bounds != Some((size.width, ui_height)) {
            let _ = self.ui.set_bounds(bounds(0., 0., size.width, ui_height));
            self.ui_bounds = Some((size.width, ui_height));
        }
        // Clip chrome to the toolbar and dropdown, leaving the page beneath it
        // at its original size and interactive everywhere outside the dropdown.
        let pending = split.map(|ids| {
            ids.map(|id| {
                self.tabs
                    .iter()
                    .find(|t| t.id == id)
                    .is_none_or(|t| !t.presented)
            })
        });
        let region_state = format!(
            "{showing_page}:{top}:{}:{}:{}:{overlay:?}:{}:{split:?}:{}:{pending:?}",
            size.width,
            size.height,
            self.window.scale_factor(),
            self.ui_overlay_radius,
            self.split_ratio
        );
        if self.ui_region.as_ref() != Some(&region_state) {
            use windows::Win32::{
                Graphics::Gdi::{
                    CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn,
                    RGN_OR,
                },
                UI::WindowsAndMessaging::{
                    SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                },
            };
            unsafe {
                let mut hwnd = windows::Win32::Foundation::HWND::default();
                if self.ui.controller().ParentWindow(&mut hwnd).is_ok() {
                    if showing_page && (overlay.is_some() || split.is_some()) {
                        let scale = self.window.scale_factor();
                        let region = CreateRectRgn(
                            0,
                            0,
                            (size.width * scale).ceil() as i32,
                            (top * scale).ceil() as i32,
                        );
                        if split.is_some() {
                            let divider = CreateRectRgn(
                                (split_x * scale).round() as i32,
                                (top * scale).round() as i32,
                                ((split_x + 8.) * scale).round() as i32,
                                (size.height * scale).round() as i32,
                            );
                            CombineRgn(Some(region), Some(region), Some(divider), RGN_OR);
                            let _ = DeleteObject(divider.into());
                            for (side, waiting) in
                                pending.unwrap_or([false; 2]).into_iter().enumerate()
                            {
                                if waiting {
                                    let (left, right) = if side == 0 {
                                        (0., split_x)
                                    } else {
                                        (split_x + 8., size.width)
                                    };
                                    let pane = CreateRectRgn(
                                        (left * scale).round() as i32,
                                        (top * scale).round() as i32,
                                        (right * scale).round() as i32,
                                        (size.height * scale).round() as i32,
                                    );
                                    CombineRgn(Some(region), Some(region), Some(pane), RGN_OR);
                                    let _ = DeleteObject(pane.into());
                                }
                            }
                        }
                        if let Some((x, y, w, h)) = overlay {
                            let dropdown = if self.ui_overlay_radius > 0. {
                                let radius = (self.ui_overlay_radius * 2. * scale).round() as i32;
                                CreateRoundRectRgn(
                                    (x * scale).round() as i32,
                                    (y * scale).round() as i32,
                                    ((x + w) * scale).round() as i32,
                                    ((y + h) * scale).round() as i32,
                                    radius,
                                    radius,
                                )
                            } else {
                                CreateRectRgn(
                                    ((x - 4.) * scale).floor() as i32,
                                    ((y - 4.) * scale).floor() as i32,
                                    ((x + w + 4.) * scale).ceil() as i32,
                                    ((y + h + 4.) * scale).ceil() as i32,
                                )
                            };
                            CombineRgn(Some(region), Some(region), Some(dropdown), RGN_OR);
                            let _ = DeleteObject(dropdown.into());
                        }
                        if SetWindowRgn(hwnd, Some(region), true) == 0 {
                            let _ = DeleteObject(region.into());
                        }
                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOP),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    } else {
                        SetWindowRgn(hwnd, None, true);
                    }
                }
            }
            self.ui_region = Some(region_state);
        }
        for t in &mut self.tabs {
            let Some(view) = &t.view else {
                continue;
            };
            let (x, width) = if let Some(ids) = split {
                if ids[0] == t.id {
                    (0., split_x)
                } else if ids[1] == t.id {
                    (split_x + 8., (size.width - split_x - 8.).max(1.))
                } else {
                    (0., size.width)
                }
            } else {
                (0., size.width)
            };
            let dimensions = (x, top, width, size.height - top);
            if t.last_bounds != Some(dimensions) {
                let _ = view.set_bounds(bounds(x, top, width, size.height - top));
                t.last_bounds = Some(dimensions);
            }
            let visible = (t.id == self.active || split.is_some_and(|ids| ids.contains(&t.id)))
                && t.presented
                && t.url != "azy://newtab"
                && self.panel.is_empty()
                && !self.window.is_minimized();
            if visible != t.visible {
                if visible {
                    if let Ok(native) = view.webview().cast::<ICoreWebView2_3>() {
                        unsafe {
                            let _ = native.Resume();
                        }
                    }
                    t.suspended = false;
                } else {
                    t.hidden_since = Instant::now();
                }
                let _ = view.set_visible(visible);
                t.visible = visible;
            }
        }
        // Showing a previously hidden child can change its z-order. Keep chrome's
        // clipped overlay and divider above the website children after that change.
        if showing_page && (overlay.is_some() || split.is_some()) {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            };
            let mut hwnd = windows::Win32::Foundation::HWND::default();
            unsafe {
                if self.ui.controller().ParentWindow(&mut hwnd).is_ok() {
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                    );
                }
            }
        }
        if let Some(popup) = &self.extension_popup {
            let width = 420_f64.min(size.width - 16.);
            let height = 560_f64.min(size.height - top - 8.);
            let _ = popup.set_bounds(bounds(
                (size.width - width - 8.).max(0.),
                top,
                width,
                height,
            ));
            let _ = popup.set_visible(!self.window.is_minimized());
        }
    }
    fn present_ready(&mut self) {
        let now = Instant::now();
        let mut changed = false;
        for tab in &mut self.tabs {
            if tab.reveal_at.is_some_and(|deadline| now >= deadline) {
                tab.presented = true;
                tab.reveal_at = None;
                changed = true;
            }
        }
        if changed {
            self.layout();
            self.sync();
        }
    }
    fn update_backgrounds(&self) {
        let color = browser_background(self.store.settings.dark);
        self.window.set_background_color(Some(color));
        let _ = self.ui.set_background_color(color);
    }
    fn check_update(&mut self) {
        self.next_update_check = Instant::now() + Duration::from_secs(6 * 60 * 60);
        if matches!(
            self.update.status.as_str(),
            "checking" | "downloading" | "ready" | "installing"
        ) {
            return;
        }
        self.update.status = "checking".into();
        self.update.message = "Memeriksa rilis terbaru…".into();
        self.sync();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = updater::check().map_err(|e| e.to_string());
            let _ = proxy.send_event(Msg::UpdateChecked(result));
        });
    }
    fn download_update(&mut self) {
        if matches!(
            self.update.status.as_str(),
            "checking" | "downloading" | "ready" | "installing"
        ) {
            return;
        }
        let Some(available) = self.update_available.clone() else {
            self.check_update();
            return;
        };
        self.update.status = "downloading".into();
        self.update.progress = 0;
        self.update.message = "Mengunduh dan memverifikasi pembaruan…".into();
        self.sync();
        let root = self.root.clone();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = updater::download(&root, &available, &proxy).map_err(|e| e.to_string());
            let _ = proxy.send_event(Msg::UpdateDownloaded(result));
        });
    }
    fn install_update(&mut self) {
        let Some(plan) = self.update_plan.clone() else {
            return;
        };
        if self
            .store
            .downloads
            .iter()
            .any(|d| d.status == "downloading")
        {
            self.toast("Tunggu unduhan selesai sebelum memulai ulang untuk pembaruan.");
            return;
        }
        if instance::has_other_windows(&self.root) {
            self.toast("Tutup jendela Azy Core lain pada profil ini sebelum memperbarui.");
            return;
        }
        // Another window may have saved its own session after our cached snapshot.
        // Persist this window's session explicitly before the updater restarts it.
        self.last_saved = None;
        self.save();
        if self.last_saved.is_none() {
            return;
        }
        match updater::install(&plan) {
            Ok(()) => {
                self.update.status = "installing".into();
                self.sync();
                self.exit = true;
            }
            Err(error) => {
                self.update.status = "error".into();
                self.update.message = error.to_string();
                self.sync();
            }
        }
    }
    fn maintain(&mut self) {
        self.next_maintenance = Instant::now() + Duration::from_secs(5);
        if Instant::now() >= self.next_update_check {
            self.check_update();
        }
        self.sync();
        if !self.store.settings.memory_saver
            || self
                .store
                .downloads
                .iter()
                .any(|d| d.status == "downloading")
        {
            return;
        }
        for tab in &mut self.tabs {
            if tab.id == self.active
                || tab.visible
                || tab.loading
                || tab.suspended
                || tab.suspend_pending
                || tab.hidden_since.elapsed() < Duration::from_secs(60)
            {
                continue;
            }
            let Some(view) = &tab.view else {
                continue;
            };
            // WebView2 also declines suspension for protected activities such as WebRTC.
            if let Ok(audio) = view.webview().cast::<ICoreWebView2_8>() {
                let mut playing = BOOL(0);
                if unsafe { audio.IsDocumentPlayingAudio(&mut playing) }.is_err()
                    || playing.as_bool()
                {
                    continue;
                }
            }
            if let Ok(native) = view.webview().cast::<ICoreWebView2_3>() {
                let id = tab.id;
                let proxy = self.proxy.clone();
                let completed =
                    TrySuspendCompletedHandler::create(Box::new(move |result, success| {
                        let _ = proxy.send_event(Msg::Suspended(id, result.is_ok() && success));
                        Ok(())
                    }));
                tab.suspend_pending = unsafe { native.TrySuspend(&completed) }.is_ok();
                tab.hidden_since = Instant::now();
            }
        }
    }
    fn save(&mut self) {
        self.store.session_pins = self
            .tabs
            .iter()
            .filter(|t| !t.private)
            .map(|t| t.pinned)
            .collect();
        self.store.session = self
            .tabs
            .iter()
            .filter(|t| !t.private)
            .map(|t| t.url.clone())
            .collect();
        let snapshot = serde_json::to_vec(&self.store).unwrap_or_default();
        if self.last_saved.as_ref() == Some(&snapshot) {
            return;
        }
        if let Err(e) = self.store.save(&self.root.join("browser.json")) {
            self.toast(&format!("Gagal menyimpan data: {e}"));
        } else {
            self.last_saved = Some(snapshot);
        }
    }
    fn sync(&self) {
        let tabs: Vec<Value> = self.tabs.iter().map(|t| {
            let (mut back, mut forward) = (BOOL(0), BOOL(0));
            if let Some(view) = &t.view { unsafe { let _ = view.webview().CanGoBack(&mut back); let _ = view.webview().CanGoForward(&mut forward); } }
            json!({"id":t.id,"title":t.title,"url":t.url,"private":t.private,"pinned":t.pinned,"loading":t.loading,"back":back.as_bool(),"forward":forward.as_bool(),"zoom":(t.zoom*100.).round(),"favicon":t.favicon,"suspended":t.suspended,"loaded":t.view.is_some(),"presentation_pending":t.view.is_some() && !t.presented && t.url != "azy://newtab","blocked":t.network.borrow().blocked,"adblock":self.blocker.borrow().active_for(&t.url)})
        }).collect();
        let extension_rows: Vec<Value> = self.extension_list.iter().map(|info| json!({"id":info.id,"name":info.name,"enabled":info.enabled,"pinned":self.store.extension_pins.get(&info.id).copied().unwrap_or(true),"source":self.store.extensions.get(&info.id)})).collect();
        let state = json!({"tabs":tabs,"active":self.active,"split":self.split,"splitRatio":self.split_ratio,"splitVisible":self.content_fullscreen.is_none()&&self.split.is_some_and(|ids|ids.contains(&self.active))&&self.panel.is_empty(),"panel":self.panel,"home":self.store.home,"extensions":extension_rows,"extensionPopup":self.extension_popup_id,"storePrepared":self.store_prepared,"extensionBusy":self.extension_busy,"extensionReady":self.extension_ready,"extensionError":self.extension_error,"bookmarks":self.store.bookmarks,"history":self.store.history,"downloads":self.store.downloads,"settings":self.store.settings,"dataPath":self.root.display().to_string(),"contentFullscreen":self.content_fullscreen.map(|(id,_)|id),"windowFullscreen":self.window.fullscreen().is_some(),"viewportHeight":self.window.inner_size().height as f64/self.window.scale_factor(),"update":self.update,"adblockRules":self.blocker.borrow().rule_count()});
        let mut previous = self.last_ui_state.borrow_mut();
        let mut changes = serde_json::Map::new();
        for (key, value) in state.as_object().unwrap() {
            if previous.get(key) != Some(value) {
                changes.insert(key.clone(), value.clone());
            }
        }
        if !changes.is_empty() {
            if self
                .ui
                .evaluate_script(&format!("window.receive({})", Value::Object(changes)))
                .is_ok()
            {
                *previous = state;
            }
        }
        if let Some(t) = self.current() {
            self.window.set_title(&format!(
                "{} — Azy Core{}",
                t.title,
                if t.private { " (Privat)" } else { "" }
            ));
        }
    }
    fn toast(&self, s: &str) {
        let _ = self
            .ui
            .evaluate_script(&format!("window.toast({})", json!(s)));
    }
    fn close_chrome_overlays(&mut self) {
        self.ui_overlay = None;
        self.ui_overlay_radius = 0.;
        let _ = self.ui.evaluate_script("window.closeChromeOverlays?.()");
    }
    fn pin_tab(&mut self, id: u64, pin: bool) {
        self.close_chrome_overlays();
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            let mut tab = self.tabs.remove(index);
            tab.pinned = pin;
            let index = self.tabs.iter().filter(|t| t.pinned).count();
            self.tabs.insert(index, tab);
            self.align_split_tabs(Some(id));
            self.layout();
            self.sync();
            self.save();
        }
    }
    fn move_tab(&mut self, id: u64, target: u64, after: bool) {
        if id == target || !self.tabs.iter().any(|t| t.id == target) {
            return;
        }
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            let tab = self.tabs.remove(index);
            let requested =
                self.tabs.iter().position(|t| t.id == target).unwrap_or(0) + usize::from(after);
            let pins = self.tabs.iter().filter(|t| t.pinned).count();
            let index = if tab.pinned {
                requested.min(pins)
            } else {
                requested.max(pins).min(self.tabs.len())
            };
            self.tabs.insert(index, tab);
            self.align_split_tabs(Some(id));
            self.close_chrome_overlays();
            self.layout();
            self.sync();
            self.save();
        }
    }
    // Keep the pair together without changing either tab's pinned state.
    fn align_split_tabs(&mut self, anchor: Option<u64>) {
        let Some(ids) = self.split else { return };
        let Some(left) = self.tabs.iter().position(|t| t.id == ids[0]) else {
            return;
        };
        let Some(right) = self.tabs.iter().position(|t| t.id == ids[1]) else {
            return;
        };
        if self.tabs[left].pinned != self.tabs[right].pinned {
            // A mixed pair meets at the boundary between pinned and ordinary tabs.
            let pinned = if self.tabs[left].pinned {
                ids[0]
            } else {
                ids[1]
            };
            let normal = if pinned == ids[0] { ids[1] } else { ids[0] };
            for id in [pinned, normal] {
                let index = self.tabs.iter().position(|t| t.id == id).unwrap();
                let tab = self.tabs.remove(index);
                let boundary = self.tabs.iter().filter(|t| t.pinned).count();
                self.tabs.insert(boundary, tab);
            }
        } else if anchor == Some(ids[1]) {
            let tab = self.tabs.remove(left);
            let index = self.tabs.iter().position(|t| t.id == ids[1]).unwrap();
            self.tabs.insert(index, tab);
        } else {
            let tab = self.tabs.remove(right);
            let index = self.tabs.iter().position(|t| t.id == ids[0]).unwrap();
            self.tabs.insert(index + 1, tab);
        }
    }
    fn start_split(&mut self, id: u64) {
        let Some(current) = self.current() else {
            return;
        };
        if id == self.active
            || current.url == "azy://newtab"
            || !self
                .tabs
                .iter()
                .any(|t| t.id == id && t.private == current.private && t.url != "azy://newtab")
        {
            self.toast("Pilih dua tab website yang berbeda dalam mode penjelajahan yang sama.");
            return;
        }
        self.close_chrome_overlays();
        self.close_extension_popup();
        self.panel.clear();
        if let Err(error) = self.ensure_view(id) {
            self.toast(&error.to_string());
            return;
        }
        self.split = Some([self.active, id]);
        self.align_split_tabs(Some(self.active));
        self.split_ratio = 0.5;
        self.layout();
        self.sync();
        self.save();
        if let Some(view) = self.current().and_then(|t| t.view.as_ref()) {
            let _ = view.focus();
        }
    }
    fn refresh_shortcut_icons(&mut self) {
        let origins: std::collections::BTreeSet<String> = self
            .store
            .home
            .shortcuts
            .iter()
            .filter_map(|link| model::website_origin(&link.url))
            .collect();
        self.store
            .home
            .icons
            .retain(|origin, _| origins.contains(origin));
        for tab in &self.tabs {
            if !tab.private && !tab.favicon.is_empty() {
                if let Some(origin) =
                    model::website_origin(&tab.url).filter(|origin| origins.contains(origin))
                {
                    self.store.home.icons.insert(origin, tab.favicon.clone());
                }
            }
        }
    }
    fn send_home_background(&self) {
        use std::fmt::Write;
        let mut hex = String::new();
        let file = self.root.join("home-background.png");
        if self.store.home.background_image
            && fs::metadata(&file).is_ok_and(|m| m.len() <= 8 * 1024 * 1024)
        {
            if let Ok(bytes) = fs::read(file) {
                hex.reserve(bytes.len() * 2);
                for byte in bytes {
                    let _ = write!(hex, "{byte:02x}");
                }
            }
        }
        let _ = self
            .ui
            .evaluate_script(&format!("window.setHomeBackground({})", json!(hex)));
    }
    fn save_home_image(&mut self, hex: &str) -> Result<(), Box<dyn std::error::Error>> {
        if hex.is_empty() || hex.len() > 16 * 1024 * 1024 || hex.len() % 2 != 0 {
            return Err("Gambar terlalu besar atau tidak valid".into());
        }
        let bytes: Result<Vec<u8>, _> = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                std::str::from_utf8(pair)
                    .ok()
                    .and_then(|s| u8::from_str_radix(s, 16).ok())
                    .ok_or("Gambar tidak valid")
            })
            .collect();
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(bytes?), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode()?;
        let image = if image.width() > 1920 || image.height() > 1080 {
            image.resize(1920, 1080, image::imageops::FilterType::Lanczos3)
        } else {
            image
        };
        let temporary = self.root.join("home-background.tmp");
        image.save_with_format(&temporary, image::ImageFormat::Png)?;
        fs::rename(temporary, self.root.join("home-background.png"))?;
        self.store.home.background_image = true;
        self.send_home_background();
        self.save();
        self.sync();
        Ok(())
    }
    fn focus_address(&self) {
        let _ = self.ui.focus();
        let _ = self.ui.evaluate_script("window.focusAddress()");
    }
    fn navigate(&mut self, input: &str) {
        self.leave_content_fullscreen();
        if address(input, &self.store.settings.search) == "azy://newtab"
            && self.split.is_some_and(|ids| ids.contains(&self.active))
        {
            self.split = None;
        }
        self.close_chrome_overlays();
        self.close_extension_popup();
        let u = address(input, &self.store.settings.search);
        self.panel.clear();
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == self.active) {
            t.navigation_id = None;
            if t.url == "azy://newtab" && u != "azy://newtab" && t.view.is_some() {
                t.presented = false;
                t.reveal_at = Some(Instant::now() + Duration::from_secs(3));
            }
            t.set_url(u.clone());
            if u == "azy://newtab" {
                t.title = "Tab baru".into();
                t.loading = false;
                t.presented = false;
                t.reveal_at = None;
                if let Some(view) = &t.view {
                    let _ = view.load_url("about:blank");
                }
            } else {
                t.loading = true;
                if let Some(view) = &t.view {
                    if let Err(e) = view.load_url(&u) {
                        t.loading = false;
                        self.toast(&e.to_string());
                    }
                }
            }
        }
        if let Err(e) = self.ensure_view(self.active) {
            self.toast(&e.to_string());
        }
        self.layout();
        self.sync();
        self.save();
        if let Some(view) = self
            .current()
            .filter(|t| t.url != "azy://newtab")
            .and_then(|t| t.view.as_ref())
        {
            let _ = view.focus();
        }
    }
    fn close_tab(&mut self, id: u64) {
        self.leave_content_fullscreen();
        self.close_chrome_overlays();
        self.close_extension_popup();
        let survivor = self.split.filter(|ids| ids.contains(&id)).map(|ids| {
            if ids[0] == id {
                ids[1]
            } else {
                ids[0]
            }
        });
        if survivor.is_some() {
            self.split = None;
        }
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            let t = self.tabs.remove(i);
            self.closed.push((t.url, t.private, t.pinned));
            if self.active == id {
                self.active = survivor
                    .filter(|other| self.tabs.iter().any(|t| t.id == *other))
                    .unwrap_or_else(|| {
                        self.tabs
                            .get(i.min(self.tabs.len().saturating_sub(1)))
                            .map(|t| t.id)
                            .unwrap_or(0)
                    });
            }
            if self.tabs.is_empty() {
                if let Err(e) = self.new_tab("", false) {
                    self.toast(&e.to_string());
                }
            }
            self.panel.clear();
            if let Err(e) = self.ensure_view(self.active) {
                self.toast(&e.to_string());
            }
            self.layout();
            self.sync();
            self.save();
        }
    }
    fn close_extension_popup(&mut self) {
        self.extension_popup_generation += 1;
        self.extension_popup_id.clear();
        if let Some(view) = self.extension_popup.take() {
            let _ = view.set_visible(false);
        }
    }
    fn open_extension_popup(&mut self, id: &str) {
        self.close_chrome_overlays();
        self.layout();
        let was_open = self.extension_popup_id == id;
        self.close_extension_popup();
        self.sync();
        if was_open {
            return;
        }
        if !self.extension_list.iter().any(|e| e.id == id && e.enabled) {
            return;
        }
        if self
            .current()
            .is_none_or(|t| t.private || t.url == "azy://newtab")
        {
            self.toast("Buka website di tab biasa untuk menggunakan ekstensi.");
            return;
        }
        if self
            .store
            .extensions
            .get(id)
            .and_then(|s| s.popup.as_ref())
            .is_none()
        {
            self.toast("Ekstensi ini tidak menyediakan popup toolbar.");
            return;
        }
        let generation = self.extension_popup_generation;
        let tab = self.active;
        let id = id.to_string();
        let proxy = self.proxy.clone();
        let callback =
            CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |status, result| {
                if status.is_ok() {
                    if let Ok(value) = serde_json::from_str::<Value>(&result) {
                        if let Some(window) = value["windowId"].as_i64() {
                            let _ = proxy.send_event(Msg::ExtensionPopupReady(
                                generation,
                                tab,
                                id.clone(),
                                window,
                            ));
                            return Ok(());
                        }
                    }
                }
                let _ = proxy.send_event(Msg::ExtensionError(
                    "Tidak dapat menentukan tab aktif untuk popup ekstensi.".into(),
                ));
                Ok(())
            }));
        if let Some(view) = self.current().and_then(|t| t.view.as_ref()) {
            let method: Vec<u16> = "Browser.getWindowForTarget"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let params: Vec<u16> = "{}".encode_utf16().chain(Some(0)).collect();
            if let Err(error) = unsafe {
                view.webview().CallDevToolsProtocolMethod(
                    windows::core::PCWSTR(method.as_ptr()),
                    windows::core::PCWSTR(params.as_ptr()),
                    &callback,
                )
            } {
                self.toast(&error.to_string());
            }
        }
    }
    fn show_extension_popup(&mut self, generation: u64, tab: u64, id: String, window: i64) {
        if generation != self.extension_popup_generation
            || tab != self.active
            || self.current().is_none_or(|t| t.private)
        {
            return;
        }
        let Some(target) = self
            .store
            .extensions
            .get(&id)
            .and_then(|s| s.popup.as_ref())
            .and_then(|p| extensions::page_url(&id, p))
        else {
            return;
        };
        let network = Rc::new(RefCell::new(adblock::Page::default()));
        match self.create_view_with_popup(0, &target, false, network, Some((&id, window))) {
            Ok(view) => {
                let proxy = self.proxy.clone();
                let close = WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
                    let _ = proxy.send_event(Msg::ExtensionPopupClose(generation));
                    Ok(())
                }));
                unsafe {
                    let mut token = 0;
                    let _ = view.webview().add_WindowCloseRequested(&close, &mut token);
                }
                self.panel.clear();
                self.extension_popup = Some(view);
                self.extension_popup_id = id;
                self.layout();
                self.sync();
                if let Some(view) = &self.extension_popup {
                    let _ = view.focus();
                }
            }
            Err(error) => self.toast(&error.to_string()),
        }
    }
    fn profile_host(&mut self) -> wry::Result<&WebView> {
        if self.extension_host.is_none() {
            let downloads = self.download_folder();
            let _ = fs::create_dir_all(&downloads);
            let host = self.create_view(
                0,
                "about:blank",
                false,
                Rc::new(RefCell::new(adblock::Page::default())),
            )?;
            // Background extension downloads use the selected download directory,
            // including downloads initiated by an offscreen document.
            if let Ok(native) = host.webview().cast::<ICoreWebView2_13>() {
                unsafe {
                    if let Ok(profile) = native.Profile() {
                        let path: Vec<u16> = downloads
                            .to_string_lossy()
                            .encode_utf16()
                            .chain(Some(0))
                            .collect();
                        let _ = profile
                            .SetDefaultDownloadFolderPath(windows::core::PCWSTR(path.as_ptr()));
                    }
                }
            }
            self.extension_host = Some(host);
        }
        Ok(self.extension_host.as_ref().unwrap())
    }
    fn ensure_extensions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.extension_manager.is_none() {
            let manager = extensions::Manager::new(self.profile_host()?).map_err(|error| {
                format!("Runtime WebView2 belum mendukung pengelolaan ekstensi. {error}")
            })?;
            self.extension_manager = Some(manager);
        }
        Ok(())
    }
    fn refresh_extensions(&mut self) {
        self.extension_busy = true;
        self.sync();
        let result = self.ensure_extensions().and_then(|_| {
            self.extension_manager
                .as_ref()
                .unwrap()
                .refresh(self.proxy.clone())
                .map_err(Into::into)
        });
        if let Err(error) = result {
            self.extension_busy = false;
            self.extension_error = error.to_string();
        }
        self.sync();
    }
    fn install_extension(&mut self, folder: &str) {
        if self.extension_busy {
            return;
        }
        self.extension_error.clear();
        self.extension_busy = true;
        let result = self.ensure_extensions().and_then(|_| {
            self.extension_manager
                .as_ref()
                .unwrap()
                .install(folder, self.proxy.clone())
        });
        if let Err(error) = result {
            self.extension_busy = false;
            self.extension_error = error.to_string();
        }
        self.sync();
    }
    fn prepare_store(&mut self, input: &str) {
        if self.extension_busy {
            return;
        }
        self.close_extension_popup();
        self.close_chrome_overlays();
        self.panel = "extensions".into();
        self.layout();
        if let Some(old) = self.store_prepared.take() {
            webstore::discard(&self.root, &old);
        }
        self.extension_error.clear();
        let Some(id) = webstore::store_id(input) else {
            self.extension_error =
                "Masukkan URL halaman detail Chrome Web Store atau ID ekstensi yang valid.".into();
            self.sync();
            return;
        };
        if self.extension_list.iter().any(|e| e.id == id) {
            self.extension_error =
                "Ekstensi sudah terpasang. Hapus dahulu jika ingin memasang ulang versi terbaru."
                    .into();
            self.sync();
            return;
        }
        let mut raw = windows::core::PWSTR::null();
        let version = unsafe {
            if self.ui.environment().BrowserVersionString(&mut raw).is_ok() {
                webview2_com::take_pwstr(raw)
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            }
        };
        if version.is_empty() {
            self.extension_error = "Versi runtime WebView2 tidak dapat dibaca.".into();
            self.sync();
            return;
        }
        self.extension_busy = true;
        self.sync();
        let root = self.root.clone();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = webstore::prepare(&root, &id, &version).map_err(|e| e.to_string());
            let _ = proxy.send_event(Msg::StorePrepared(result));
        });
    }
    fn confirm_store(&mut self) {
        if self.extension_busy {
            return;
        }
        let Some(package) = self.store_prepared.take() else {
            return;
        };
        self.extension_busy = true;
        self.extension_error.clear();
        self.sync();
        let result = self.ensure_extensions().and_then(|_| {
            self.extension_manager
                .as_ref()
                .unwrap()
                .install_source(package.source, self.proxy.clone())
        });
        if let Err(error) = result {
            self.extension_busy = false;
            self.extension_error = error.to_string();
            self.sync();
        }
    }
    fn download_folder(&self) -> PathBuf {
        if self.store.settings.download_directory.is_empty() {
            self.root.join("Downloads")
        } else {
            PathBuf::from(&self.store.settings.download_directory)
        }
    }
    fn choose_download_folder(
        &self,
        name: Option<&str>,
        remember: bool,
    ) -> Option<(PathBuf, bool)> {
        use tao::platform::windows::WindowExtWindows;
        match downloads::choose_folder(
            &self.download_folder(),
            name,
            remember,
            windows::Win32::Foundation::HWND(self.window.hwnd() as *mut _),
        ) {
            Ok(folder) => folder,
            Err(error) => {
                self.toast(&format!("Tidak dapat memilih folder unduhan: {error}"));
                None
            }
        }
    }
    fn remember_download_folder(&mut self, folder: PathBuf, automatic: bool) {
        self.store.settings.download_directory = folder.to_string_lossy().into_owned();
        self.store.settings.download_ask = !automatic;
        self.update_download_profile();
        self.save();
        self.sync();
    }
    fn begin_download(&mut self, id: u64) {
        diagnostic(&format!("DownloadRequested {id}"));
        let Some(mut pending) = self.downloads.take(id) else {
            return;
        };
        self.close_chrome_overlays();
        let folder = self.download_folder();
        let selection = if self.store.settings.download_ask {
            self.choose_download_folder(Some(&pending.name), false)
        } else if downloads::prepare_folder(&folder, id).is_ok() {
            Some((folder, true))
        } else {
            self.toast("Folder unduhan tidak dapat digunakan. Pilih folder lain.");
            self.choose_download_folder(Some(&pending.name), false)
        };
        let Some((folder, automatic)) = selection else {
            return;
        };
        if let Err(error) = downloads::prepare_folder(&folder, id) {
            self.toast(&format!("Folder unduhan tidak dapat ditulis: {error}"));
            return;
        }
        self.remember_download_folder(folder.clone(), automatic);
        let reserved: Vec<PathBuf> = self
            .store
            .downloads
            .iter()
            .filter(|d| d.status == "downloading")
            .map(|d| PathBuf::from(&d.path))
            .collect();
        let path = downloads::target(&folder, &pending.name, &reserved);
        self.store.downloads.push(Download {
            id,
            url: pending.url.clone(),
            path: path.to_string_lossy().into_owned(),
            status: "downloading".into(),
            time: now(),
            received: 0,
            total: 0,
            seen: false,
        });
        if let Err(error) = pending.start(&path, id, self.proxy.clone()) {
            if let Some(download) = self.store.downloads.iter_mut().find(|d| d.id == id) {
                download.status = "failed".into();
            }
            self.toast(&format!("Unduhan tidak dapat dimulai: {error}"));
        }
        self.save();
        self.sync();
    }
    fn update_download_profile(&self) {
        let view = self.extension_host.as_ref().or_else(|| {
            self.tabs
                .iter()
                .filter(|t| !t.private)
                .find_map(|t| t.view.as_ref())
        });
        if let Some(view) = view {
            if let Ok(native) = view.webview().cast::<ICoreWebView2_13>() {
                unsafe {
                    if let Ok(profile) = native.Profile() {
                        let _ = profile.SetDefaultDownloadFolderPath(
                            &windows::core::HSTRING::from(self.download_folder().as_os_str()),
                        );
                    }
                }
            }
        }
    }
    fn pick_extension_folder(&mut self) {
        use windows::Win32::{
            System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            UI::Shell::{
                FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
                SIGDN_FILESYSPATH,
            },
        };
        let result: windows::core::Result<String> = unsafe {
            (|| {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
                dialog.SetOptions(dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)?;
                dialog.Show(None)?;
                let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
                Ok(webview2_com::take_pwstr(path))
            })()
        };
        if let Ok(folder) = result {
            self.install_extension(&folder);
        }
    }
    fn message(&mut self, msg: Msg) {
        match msg {
            Msg::ContentFullscreen(id, full) => {
                if full && self.tabs.iter().any(|t| t.id == id && t.visible) {
                    self.close_chrome_overlays();
                    self.close_extension_popup();
                    if self.content_fullscreen.is_none() {
                        self.content_fullscreen = Some((id, self.window.fullscreen().is_some()));
                        self.active = id;
                        if self.store.settings.video_fullscreen {
                            self.window
                                .set_fullscreen(Some(Fullscreen::Borderless(None)));
                        }
                    }
                    self.layout();
                    if let Some(view) = self.current().and_then(|t| t.view.as_ref()) {
                        let _ = view.focus();
                    }
                    self.sync();
                } else if !full
                    && self
                        .content_fullscreen
                        .is_some_and(|(current, _)| current == id)
                {
                    self.leave_content_fullscreen();
                    self.layout();
                    self.sync();
                }
            }
            Msg::DownloadProgress(id, received, total) => {
                if let Some(download) = self.store.downloads.iter_mut().find(|d| d.id == id) {
                    download.received = received;
                    download.total = total;
                    self.sync();
                }
            }
            Msg::UpdateChecked(result) => {
                match result {
                    Ok(Some(available)) => {
                        let notify = self.update.version != available.manifest.version;
                        self.update.version = available.manifest.version.clone();
                        self.update.status = "available".into();
                        self.update.message = format!("Azy Core {} tersedia.", self.update.version);
                        self.update_available = Some(available);
                        if notify {
                            self.toast(&self.update.message);
                        }
                    }
                    Ok(None) => {
                        self.update.status = "current".into();
                        self.update.version.clear();
                        self.update.message = "Azy Core sudah menggunakan versi terbaru.".into();
                        self.update_available = None;
                    }
                    Err(error) => {
                        self.update.status = "error".into();
                        self.update.message = error;
                    }
                }
                self.sync();
            }
            Msg::UpdateProgress(progress) => {
                self.update.progress = progress;
                self.sync();
            }
            Msg::UpdateDownloaded(result) => {
                match result {
                    Ok(plan) => {
                        self.update_plan = Some(plan);
                        self.update.status = "ready".into();
                        self.update.progress = 100;
                        self.update.message =
                            "Pembaruan siap dipasang. Mulai ulang Azy Core untuk menerapkan."
                                .into();
                    }
                    Err(error) => {
                        self.update.status = "error".into();
                        self.update.message = error;
                    }
                }
                self.sync();
            }
            Msg::ExternalOpen(inputs, private) => {
                for input in inputs {
                    if let Err(error) = self.new_tab(&input, private) {
                        self.toast(&format!("Tidak dapat membuka file: {error}"));
                    }
                }
                self.window.set_visible(true);
                self.window.set_minimized(false);
                self.window.set_focus();
            }
            Msg::ContentFocused(id) => {
                if id != self.active
                    && self
                        .split
                        .is_some_and(|ids| ids.contains(&self.active) && ids.contains(&id))
                    && self.tabs.iter().any(|t| t.id == id && t.visible)
                {
                    self.close_chrome_overlays();
                    self.close_extension_popup();
                    self.active = id;
                    self.layout();
                    self.sync();
                }
            }
            Msg::StorePrepared(result) => {
                self.extension_busy = false;
                match result {
                    Ok(package) => self.store_prepared = Some(package),
                    Err(error) => self.extension_error = error,
                }
                self.sync();
            }
            Msg::ExtensionPopupDismiss => {
                if self.ui_overlay.is_some() {
                    self.close_chrome_overlays();
                    self.layout();
                }
                if self.extension_popup.is_some() {
                    self.close_extension_popup();
                    self.sync();
                }
            }
            Msg::ExtensionPopupReady(generation, tab, id, window) => {
                self.show_extension_popup(generation, tab, id, window)
            }
            Msg::ExtensionPopupClose(generation) => {
                if generation == self.extension_popup_generation {
                    self.close_extension_popup();
                    self.sync();
                }
            }
            Msg::ExtensionList(rows) => {
                for info in &rows {
                    if let Some(source) = self.store.extensions.get_mut(&info.id) {
                        if let Ok(mut updated) =
                            extensions::read_source(std::path::Path::new(&source.folder))
                        {
                            updated.store_id = source.store_id.clone();
                            *source = updated;
                        }
                    }
                }
                self.extension_list = rows;
                self.extension_busy = false;
                self.extension_ready = true;
                self.save();
                self.sync();
            }
            Msg::ExtensionInstalled(id, source) => {
                self.store.extensions.insert(id, source);
                self.save();
                self.refresh_extensions();
                self.toast("Ekstensi dipasang. Muat ulang tab untuk menerapkan.");
            }
            Msg::ExtensionDone(removed) => {
                if let Some(id) = removed {
                    self.store.extensions.remove(&id);
                    self.store.extension_pins.remove(&id);
                    self.save();
                }
                self.refresh_extensions();
            }
            Msg::ExtensionError(error) => {
                self.extension_busy = false;
                self.extension_error = error;
                self.sync();
            }
            Msg::Ui(s) => {
                if let Ok(v) = serde_json::from_str::<Value>(&s) {
                    let command = v["cmd"].as_str().unwrap_or("");
                    let text = v["value"].as_str().unwrap_or("");
                    match command {
                        "move-tab" => {
                            if let (Some(id), Some(target)) =
                                (v["id"].as_u64(), v["target"].as_u64())
                            {
                                self.move_tab(id, target, v["after"].as_bool().unwrap_or(false));
                            }
                        }
                        "split-with" => {
                            if let Some(id) = v["id"].as_u64() {
                                self.start_split(id);
                            }
                        }
                        "split-ratio" => {
                            if let Some(ratio) = v["value"].as_f64().filter(|r| r.is_finite()) {
                                self.split_ratio = ratio.clamp(0.25, 0.75);
                                self.layout();
                                self.sync();
                            }
                        }
                        "extension-pin" => {
                            if self.extension_list.iter().any(|e| e.id == text) {
                                self.store
                                    .extension_pins
                                    .insert(text.into(), v["pinned"].as_bool().unwrap_or(true));
                                self.save();
                                self.sync();
                            }
                        }
                        "extension-menu" => {
                            self.close_extension_popup();
                            self.sync();
                            if !self.extension_ready && !self.extension_busy {
                                self.refresh_extensions();
                            }
                            let _ = self.ui.evaluate_script("window.toggleExtensionMenu()");
                        }
                        "browser-menu" | "adblock-menu" => {
                            self.close_extension_popup();
                            self.sync();
                            let script = if command == "browser-menu" {
                                "window.toggleBrowserMenu()"
                            } else {
                                "window.toggleBlockerMenu()"
                            };
                            let _ = self.ui.evaluate_script(script);
                        }
                        "download-seen" => {
                            let mut changed = false;
                            for download in &mut self.store.downloads {
                                if download.status != "downloading" && !download.seen {
                                    download.seen = true;
                                    changed = true;
                                }
                            }
                            if changed {
                                self.save();
                                self.sync();
                            }
                        }
                        "download-menu" => {
                            self.close_extension_popup();
                            self.sync();
                            let _ = self.ui.evaluate_script("window.toggleDownloadMenu()");
                        }
                        "download-show-folder" => {
                            self.close_chrome_overlays();
                            if let Some(download) = v["index"]
                                .as_u64()
                                .and_then(|i| self.store.downloads.get(i as usize))
                            {
                                let path = PathBuf::from(&download.path);
                                if let Some(folder) = path.parent().filter(|p| p.is_dir()) {
                                    let _ = std::process::Command::new("explorer.exe")
                                        .arg(folder)
                                        .spawn();
                                } else {
                                    self.toast("Folder unduhan tidak ditemukan.");
                                }
                            }
                            self.layout();
                        }
                        "extension-store" => self.prepare_store(text),
                        "extension-store-confirm" => self.confirm_store(),
                        "extension-store-cancel" => {
                            if !self.extension_busy {
                                if let Some(package) = self.store_prepared.take() {
                                    webstore::discard(&self.root, &package);
                                }
                                self.sync();
                            }
                        }
                        "extension-store-browse" => {
                            if let Err(error) =
                                self.new_tab("https://chromewebstore.google.com/", false)
                            {
                                self.toast(&error.to_string());
                            }
                        }
                        "ui-overlay" => {
                            self.ui_overlay_radius = v["value"]["radius"]
                                .as_f64()
                                .filter(|r| r.is_finite())
                                .unwrap_or(0.)
                                .clamp(0., 24.);
                            self.ui_overlay = (|| {
                                let o = &v["value"];
                                let (x, y, w, h) = (
                                    o["x"].as_f64()?,
                                    o["y"].as_f64()?,
                                    o["width"].as_f64()?,
                                    o["height"].as_f64()?,
                                );
                                if [x, y, w, h].iter().all(|n| n.is_finite())
                                    && x >= 0.
                                    && y >= 0.
                                    && w > 0.
                                    && h > 0.
                                    && h <= 600.
                                {
                                    Some((x, y, w.min(4096.), h))
                                } else {
                                    None
                                }
                            })();
                            self.layout();
                        }
                        "pin-tab" => {
                            if let Some(id) = v["id"].as_u64() {
                                self.pin_tab(id, v["pinned"].as_bool().unwrap_or(true));
                            }
                        }
                        "extension-popup" => self.open_extension_popup(text),
                        "extension-install" => self.install_extension(text),
                        "extension-pick" => {
                            if !self.extension_busy {
                                self.pick_extension_folder();
                            }
                        }
                        "extensions-refresh" => {
                            if !self.extension_busy {
                                self.extension_error.clear();
                                self.refresh_extensions();
                            }
                        }
                        "extension-enable" | "extension-remove" => {
                            self.close_extension_popup();
                            if !self.extension_busy {
                                if let Some(manager) = &self.extension_manager {
                                    self.extension_busy = true;
                                    self.extension_error.clear();
                                    let result = if command == "extension-enable" {
                                        manager.enable(
                                            text,
                                            v["enabled"].as_bool().unwrap_or(false),
                                            self.proxy.clone(),
                                        )
                                    } else {
                                        manager.remove(text, self.proxy.clone())
                                    };
                                    if let Err(error) = result {
                                        self.extension_busy = false;
                                        self.extension_error = error.to_string();
                                    }
                                    self.sync();
                                }
                            }
                        }
                        "extension-options" => {
                            if let Some(target) = self
                                .store
                                .extensions
                                .get(text)
                                .and_then(|source| source.page.as_ref())
                                .and_then(|page| extensions::page_url(text, page))
                            {
                                if let Err(error) = self.new_tab(&target, false) {
                                    self.toast(&error.to_string());
                                }
                            }
                        }
                        "ready" => {
                            *self.last_ui_state.borrow_mut() = Value::Null;
                            self.sync();
                            self.send_home_background();
                        }
                        "save-shortcut" => {
                            let title = v["title"].as_str().unwrap_or("").trim();
                            let target = model::shortcut_url(text);
                            if title.is_empty() || title.chars().count() > 60 || target.is_none() {
                                self.toast(
                                    "Isi nama dan alamat website yang valid (http atau https).",
                                );
                            } else {
                                let shortcut = model::Shortcut {
                                    title: title.into(),
                                    url: target.unwrap(),
                                };
                                if let Some(index) = v["index"].as_u64() {
                                    if let Some(existing) =
                                        self.store.home.shortcuts.get_mut(index as usize)
                                    {
                                        *existing = shortcut;
                                    }
                                } else if self.store.home.shortcuts.len() < 100 {
                                    self.store.home.shortcuts.push(shortcut);
                                } else {
                                    self.toast("Maksimal 100 pintasan.");
                                }
                                self.refresh_shortcut_icons();
                                self.save();
                                self.sync();
                            }
                        }
                        "remove-shortcut" => {
                            if let Some(index) = v["index"]
                                .as_u64()
                                .filter(|index| *index < self.store.home.shortcuts.len() as u64)
                            {
                                self.store.home.shortcuts.remove(index as usize);
                                self.refresh_shortcut_icons();
                                self.save();
                                self.sync();
                            }
                        }
                        "home-color" => {
                            if text.is_empty()
                                || (text.len() == 7
                                    && text.starts_with('#')
                                    && text[1..].bytes().all(|c| c.is_ascii_hexdigit()))
                            {
                                self.store.home.background_color = text.into();
                                self.store.home.background_image = false;
                                let _ = fs::remove_file(self.root.join("home-background.png"));
                                self.send_home_background();
                                self.save();
                                self.sync();
                            }
                        }
                        "home-image" => {
                            if let Err(error) = self.save_home_image(text) {
                                self.toast(&format!("Gambar tidak dapat disimpan: {error}"));
                            }
                        }
                        "home-image-clear" => {
                            self.store.home.background_image = false;
                            let _ = fs::remove_file(self.root.join("home-background.png"));
                            self.send_home_background();
                            self.save();
                            self.sync();
                        }
                        "navigate" => self.navigate(text),
                        "new-url" => {
                            let private = self.current().is_some_and(|t| t.private);
                            if let Err(e) = self.new_tab(text, private) {
                                self.toast(&e.to_string());
                            }
                        }
                        "switch" => {
                            if let Some(id) = v["id"]
                                .as_u64()
                                .filter(|id| self.tabs.iter().any(|t| t.id == *id))
                            {
                                self.activate(id);
                            }
                        }
                        "close" => {
                            if let Some(id) = v["id"].as_u64() {
                                self.close_tab(id);
                            }
                        }
                        "panel" => {
                            self.panel = text.into();
                            self.layout();
                            self.sync();
                        }
                        "remove-bookmark" => {
                            self.store.bookmarks.retain(|b| b.url != text);
                            self.save();
                            self.sync();
                        }
                        "clear-history" => {
                            self.store.history.clear();
                            self.save();
                            self.sync();
                        }
                        "clear-downloads" => {
                            self.store.downloads.retain(|d| d.status == "downloading");
                            self.save();
                            self.sync();
                        }
                        "settings" => {
                            if let Ok(settings) = serde_json::from_value(v["value"].clone()) {
                                self.store.settings = settings;
                                if let Some((_, was_fullscreen)) = self.content_fullscreen {
                                    self.window.set_fullscreen(
                                        (was_fullscreen || self.store.settings.video_fullscreen)
                                            .then_some(Fullscreen::Borderless(None)),
                                    );
                                }
                                self.update_download_profile();
                                self.update_backgrounds();
                                self.update_blocker();
                                if !self.store.settings.memory_saver {
                                    for tab in &mut self.tabs {
                                        if let Some(view) = &tab.view {
                                            if let Ok(native) =
                                                view.webview().cast::<ICoreWebView2_3>()
                                            {
                                                unsafe {
                                                    let _ = native.Resume();
                                                }
                                            }
                                        }
                                        tab.suspended = false;
                                    }
                                }
                                self.layout();
                                self.save();
                                self.sync();
                            }
                        }
                        "find-text" => {
                            if let Some(view) = self.current().and_then(|t| t.view.as_ref()) {
                                let backwards = v["backwards"].as_bool().unwrap_or(false);
                                let _ = view.evaluate_script(&format!(
                                    "window.find({},false,{backwards},true,false,false,false)",
                                    json!(text)
                                ));
                            }
                        }
                        "drag" => {
                            let _ = self.window.drag_window();
                        }
                        "minimize" => self.window.set_minimized(true),
                        "maximize" => self.window.set_maximized(!self.window.is_maximized()),
                        "quit" => {
                            self.save();
                            self.exit = true;
                        }
                        _ => self.command(command),
                    }
                }
            }
            Msg::Key(command) => self.command(&command),
            Msg::New(u, private) => {
                if let Err(e) = self.new_tab(&u, private) {
                    self.toast(&e.to_string());
                }
            }
            Msg::Suspended(id, success) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.suspend_pending = false;
                    if tab.id == self.active || tab.visible || !self.store.settings.memory_saver {
                        if let Some(view) = &tab.view {
                            if let Ok(native) = view.webview().cast::<ICoreWebView2_3>() {
                                unsafe {
                                    let _ = native.Resume();
                                }
                            }
                        }
                        tab.suspended = false;
                    } else {
                        tab.suspended = success;
                    }
                    self.sync();
                }
            }
            Msg::ZoomChanged(id, factor) => {
                if factor.is_finite() && factor > 0. {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        tab.zoom = factor;
                        self.sync();
                    }
                }
            }
            Msg::FaviconChanged(id) => {
                if let Some(view) = self
                    .tabs
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.view.as_ref())
                {
                    if let (Ok(native), Ok(url)) =
                        (view.webview().cast::<ICoreWebView2_15>(), view.url())
                    {
                        let proxy = self.proxy.clone();
                        let completed =
                            GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                                let mut hex = String::new();
                                if result.is_ok() {
                                    if let Some(stream) = stream {
                                        if let Some(bytes) = favicon::read_png(&stream) {
                                            use std::fmt::Write;
                                            for b in bytes {
                                                let _ = write!(hex, "{b:02x}");
                                            }
                                        }
                                    }
                                }
                                let _ = proxy.send_event(Msg::Favicon(id, url, hex));
                                Ok(())
                            }));
                        unsafe {
                            let _ = native
                                .GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &completed);
                        }
                    }
                }
            }
            Msg::Favicon(id, url, hex) => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| {
                    t.id == id
                        && t.view.as_ref().is_some_and(|v| {
                            v.url().is_ok_and(|u| favicon::same_document(&u, &url))
                        })
                }) {
                    if !hex.is_empty() {
                        tab.favicon = hex.clone();
                        let origin = model::website_origin(&tab.url);
                        let cacheable = !tab.private;
                        if let Some(origin) = origin.filter(|origin| {
                            cacheable
                                && self.store.home.shortcuts.iter().any(|link| {
                                    model::website_origin(&link.url).as_ref() == Some(origin)
                                })
                        }) {
                            if self.store.home.icons.get(&origin) != Some(&hex) {
                                self.store.home.icons.insert(origin, hex);
                                self.save();
                            }
                        }
                        self.sync();
                    }
                }
            }
            Msg::Title(id, title) => {
                if let Some(t) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.id == id && t.url != "azy://newtab")
                {
                    t.title = title.clone();
                    if !t.private {
                        if let Some(h) = self.store.history.iter_mut().find(|h| h.url == t.url) {
                            h.title = title;
                        }
                    }
                }
                self.sync();
                self.save();
            }
            Msg::NavigationStarted(id, navigation_id, source) => {
                if self
                    .content_fullscreen
                    .is_some_and(|(current, _)| current == id)
                {
                    self.leave_content_fullscreen();
                    self.layout();
                }
                if source != "about:blank" {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        tab.navigation_id = Some(navigation_id);
                    }
                    self.message(Msg::Load(id, source, true));
                }
            }
            Msg::NavigationFinished(id, navigation_id) => {
                if let Some(tab) = self
                    .tabs
                    .iter()
                    .find(|t| t.id == id && t.navigation_id == Some(navigation_id))
                {
                    self.message(Msg::Load(id, tab.url.clone(), false));
                    self.message(Msg::FaviconChanged(id));
                }
            }
            Msg::ContentReady(id, navigation_id) => {
                if let Some(tab) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.id == id && t.navigation_id == Some(navigation_id) && !t.presented)
                {
                    let next_frame = Instant::now() + Duration::from_millis(120);
                    tab.reveal_at = Some(
                        tab.reveal_at
                            .map_or(next_frame, |deadline| deadline.min(next_frame)),
                    );
                }
            }
            Msg::Load(id, u, loading) => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                    if u != "about:blank" {
                        t.set_url(u.clone());
                        t.loading = loading;
                        if !loading && !t.presented {
                            let next_frame = Instant::now() + Duration::from_millis(120);
                            t.reveal_at = Some(
                                t.reveal_at
                                    .map_or(next_frame, |deadline| deadline.min(next_frame)),
                            );
                        }
                    }
                    if !loading
                        && !t.private
                        && (u.starts_with("https://") || u.starts_with("http://"))
                    {
                        self.store.history.retain(|h| h.url != u);
                        self.store.history.insert(
                            0,
                            Entry {
                                title: t.title.clone(),
                                url: u,
                                time: now(),
                            },
                        );
                        self.store.history.truncate(1000);
                    }
                }
                self.layout();
                self.sync();
                if !loading {
                    self.save();
                }
            }
            Msg::Refresh(id) => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                    if let Some(view) = &t.view {
                        if let Ok(u) = view.url() {
                            if u != "about:blank" {
                                t.set_url(u);
                            } else if !t.loading || t.url == "azy://newtab" {
                                // Ignore the previous new-tab blank document while a newer
                                // navigation is waiting for its response headers.
                                t.set_url("azy://newtab".into());
                                t.title = "Tab baru".into();
                                t.loading = false;
                            }
                        }
                    }
                }
                self.layout();
                self.sync();
            }
            Msg::DownloadRequested(id) => self.begin_download(id),
            Msg::DownloadFinished(id, success) => {
                if let Some(download) = self.store.downloads.iter_mut().find(|d| d.id == id) {
                    download.status = if success { "complete" } else { "failed" }.into();
                    self.toast(if success {
                        "Unduhan selesai. Lihat di menu Unduhan."
                    } else {
                        "Unduhan gagal atau dibatalkan."
                    });
                    self.save();
                    self.sync();
                }
            }
        }
    }
    fn command(&mut self, command: &str) {
        if self.content_fullscreen.is_some()
            && ![
                "escape",
                "fullscreen",
                "zoom-in",
                "zoom-out",
                "zoom-reset",
                "print",
                "stop",
            ]
            .contains(&command)
        {
            self.leave_content_fullscreen();
            self.layout();
            self.sync();
        }
        self.close_chrome_overlays();
        let popup_was_open = self.extension_popup.is_some();
        self.close_extension_popup();
        if popup_was_open {
            self.sync();
        }
        match command {
            "split-view" => {
                let _ = self.ui.evaluate_script("window.showSplitPicker()");
            }
            "split-close" => {
                self.split = None;
                self.layout();
                self.sync();
            }
            "split-swap" => {
                if let Some(ids) = &mut self.split {
                    ids.swap(0, 1);
                }
                self.align_split_tabs(None);
                self.layout();
                self.sync();
                self.save();
            }
            "extensions" => {
                self.panel = "extensions".into();
                self.layout();
                self.sync();
                if !self.extension_busy {
                    self.refresh_extensions();
                }
            }
            "toggle-site-adblock" => {
                if let Some(host) = self.current().and_then(|t| adblock::site_host(&t.url)) {
                    if self.store.settings.adblock_allowed_sites.contains(&host) {
                        self.store
                            .settings
                            .adblock_allowed_sites
                            .retain(|h| h != &host);
                    } else {
                        self.store.settings.adblock_allowed_sites.push(host);
                    }
                    self.update_blocker();
                    self.save();
                    self.panel.clear();
                    self.layout();
                    self.sync();
                    self.command("reload");
                }
            }
            "update-check" => self.check_update(),
            "update-download" => self.download_update(),
            "update-install" => self.install_update(),
            "toggle-bookmarks" => {
                self.store.settings.show_bookmarks = !self.store.settings.show_bookmarks;
                self.layout();
                self.save();
                self.sync();
            }
            "new" | "private" => {
                let private = command == "private" || self.current().is_some_and(|t| t.private);
                if let Err(e) = self.new_tab("", private) {
                    self.toast(&e.to_string());
                }
            }
            "window" => {
                let _ = std::process::Command::new(std::env::current_exe().unwrap())
                    .arg("--new-window")
                    .spawn();
            }
            "close-tab" => self.close_tab(self.active),
            "reopen" => {
                if let Some((u, private, pinned)) = self.closed.pop() {
                    if let Err(e) = self.new_tab(&u, private) {
                        self.toast(&e.to_string());
                    }
                    if pinned {
                        self.pin_tab(self.active, true);
                    }
                }
            }
            "next" | "previous" => {
                if let Some(i) = self.tabs.iter().position(|t| t.id == self.active) {
                    let n = self.tabs.len();
                    let j = (i + if command == "next" { 1 } else { n - 1 }) % n;
                    self.activate(self.tabs[j].id);
                }
            }
            "address" => self.focus_address(),
            "home" => self.navigate(""),
            "history" | "bookmarks" | "downloads" | "settings-page" | "menu" | "about"
            | "adblock" | "customize-home" => {
                self.panel = command.into();
                self.layout();
                self.sync();
                let _ = self.ui.focus();
            }
            "escape" => {
                if self.content_fullscreen.is_some() {
                    self.leave_content_fullscreen();
                    self.layout();
                    self.sync();
                    return;
                }
                self.close_chrome_overlays();
                let _ = self
                    .ui
                    .evaluate_script("document.getElementById('shortcut-dialog')?.close()");
                if self.window.fullscreen().is_some() {
                    self.window.set_fullscreen(None);
                }
                self.panel.clear();
                self.layout();
                self.sync();
                let _ = self.ui.evaluate_script("hideFind()");
            }
            "fullscreen" => {
                if self.content_fullscreen.is_some() {
                    self.leave_content_fullscreen();
                    self.window.set_fullscreen(None);
                    self.layout();
                    self.sync();
                    return;
                }
                let full = if self.window.fullscreen().is_some() {
                    None
                } else {
                    Some(Fullscreen::Borderless(None))
                };
                self.window.set_fullscreen(full);
                self.sync();
            }
            "bookmark" => {
                if let Some(t) = self.current().filter(|t| t.url != "azy://newtab") {
                    let (url, title) = (t.url.clone(), t.title.clone());
                    if self.store.bookmarks.iter().any(|b| b.url == url) {
                        self.store.bookmarks.retain(|b| b.url != url);
                    } else {
                        self.store.bookmarks.push(Entry {
                            title,
                            url,
                            time: now(),
                        });
                    }
                    self.save();
                    self.sync();
                }
            }
            "find" => {
                let _ = self.ui.focus();
                let _ = self.ui.evaluate_script("showFind()");
            }
            "open-folder" => {
                let p = self.download_folder();
                let _ = fs::create_dir_all(&p);
                let _ = std::process::Command::new("explorer.exe").arg(p).spawn();
            }
            "open-file" => self.open_file(),
            "download-folder" => {
                self.panel = "settings-page".into();
                self.layout();
                if let Some((folder, automatic)) =
                    self.choose_download_folder(None, !self.store.settings.download_ask)
                {
                    self.remember_download_folder(folder, automatic);
                }
                self.sync();
            }
            "clear-cache" => {
                let result = if let Some(view) = self
                    .tabs
                    .iter()
                    .filter(|tab| !tab.private)
                    .find_map(|tab| tab.view.as_ref())
                {
                    view.clear_all_browsing_data()
                } else {
                    self.profile_host()
                        .and_then(|view| view.clear_all_browsing_data())
                };
                match result {
                    Ok(_) => {
                        let _ = self.ui.clear_all_browsing_data();
                        self.toast("Data situs, cookies, dan cache telah diminta untuk dihapus.")
                    }
                    Err(e) => self.toast(&e.to_string()),
                }
            }
            "zoom-in" | "zoom-out" | "zoom-reset" => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == self.active) {
                    t.zoom = if command == "zoom-reset" {
                        1.
                    } else {
                        (t.zoom + if command == "zoom-in" { 0.1 } else { -0.1 }).clamp(0.25, 5.)
                    };
                    if let Some(view) = &t.view {
                        let _ = view.zoom(t.zoom);
                    }
                }
                self.sync();
            }
            "back" | "forward" | "reload" | "stop" | "print" | "devtools" => {
                if let Some(view) = self.current().and_then(|t| t.view.as_ref()) {
                    unsafe {
                        match command {
                            "back" => {
                                let _ = view.webview().GoBack();
                            }
                            "forward" => {
                                let _ = view.webview().GoForward();
                            }
                            "reload" => {
                                let _ = view.webview().Reload();
                            }
                            "stop" => {
                                let _ = view.webview().Stop();
                            }
                            "print" => {
                                let _ = view.print();
                            }
                            "devtools" => {
                                let _ = view.webview().OpenDevToolsWindow();
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fn open_file(&mut self) {
        use tao::platform::windows::WindowExtWindows;
        use windows::{
            core::{w, HSTRING},
            Win32::{
                Foundation::HWND,
                System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
                UI::Shell::{
                    Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST,
                    FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
                },
            },
        };
        let result: windows::core::Result<String> = unsafe {
            (|| -> windows::core::Result<String> {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
                dialog.SetTitle(&HSTRING::from("Buka file / PDF — Azy Core"))?;
                dialog.SetOptions(
                    dialog.GetOptions()?
                        | FOS_FILEMUSTEXIST
                        | FOS_PATHMUSTEXIST
                        | FOS_FORCEFILESYSTEM
                        | FOS_NOCHANGEDIR,
                )?;
                dialog.SetFileTypes(&[
                    COMDLG_FILTERSPEC {
                        pszName: w!("Dokumen PDF (*.pdf)"),
                        pszSpec: w!("*.pdf"),
                    },
                    COMDLG_FILTERSPEC {
                        pszName: w!("Semua file (*.*)"),
                        pszSpec: w!("*.*"),
                    },
                ])?;
                dialog.SetFileTypeIndex(1)?;
                dialog.Show(Some(HWND(self.window.hwnd() as *mut _)))?;
                let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
                let value = name.to_string();
                CoTaskMemFree(Some(name.0 as _));
                value.map_err(|_| {
                    windows::core::Error::from_hresult(windows::core::HRESULT(0x80070057u32 as i32))
                })
            })()
        };
        match result {
            Ok(path) => {
                let private = self.current().is_some_and(|t| t.private);
                if let Err(error) = self.new_tab(&path, private) {
                    self.toast(&format!("Tidak dapat membuka file: {error}"));
                }
            }
            Err(error) if error.code().0 as u32 == 0x800704c7 => {}
            Err(error) => self.toast(&format!("Tidak dapat memilih file: {error}")),
        }
    }
    fn update_blocker(&self) {
        let mut blocker = self.blocker.borrow_mut();
        blocker.enabled = self.store.settings.adblock_enabled;
        blocker.allowed_sites = self
            .store
            .settings
            .adblock_allowed_sites
            .iter()
            .cloned()
            .collect();
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let base = executable.parent().unwrap();
    let root = std::env::var_os("AZY_CORE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if base.ends_with("debug") || base.ends_with("release") {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("AzyCore.Data")
            } else {
                base.join("AzyCore.Data")
            }
        });
    fs::create_dir_all(&root)?;
    diagnostic("Profile ready");
    let event_loop = EventLoopBuilder::<Msg>::with_user_event().build();
    diagnostic("Event loop ready");
    let proxy = event_loop.create_proxy();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = match instance::start(&root, &args, proxy.clone())? {
        instance::Launch::Forwarded => return Ok(()),
        instance::Launch::Primary(instance) => Some(instance),
    };
    let mut store = Store::read(&root.join("browser.json"));
    for d in &mut store.downloads {
        if d.status == "downloading" {
            d.status = "interrupted".into();
        }
    }
    let icon_image =
        image::load_from_memory(include_bytes!(concat!(env!("OUT_DIR"), "/logo-ui.png")))?
            .resize(64, 64, image::imageops::FilterType::Lanczos3)
            .into_rgba8();
    let icon = Icon::from_rgba(
        icon_image.clone().into_raw(),
        icon_image.width(),
        icon_image.height(),
    )?;
    let window = WindowBuilder::new()
        .with_title("Azy Core")
        .with_window_icon(Some(icon))
        .with_decorations(false)
        .with_background_color(browser_background(store.settings.dark))
        .with_inner_size(LogicalSize::new(1280., 840.))
        .with_min_inner_size(LogicalSize::new(700., 480.))
        .build(&event_loop)?;
    diagnostic("Window ready");
    let context = WebContext::new(Some(root.join("WebView2")));
    let mut private_context = WebContext::new(Some(root.join("WebView2.UI")));
    let p = proxy.clone();
    let logo = {
        use std::fmt::Write;
        let mut s = String::new();
        for b in include_bytes!(concat!(env!("OUT_DIR"), "/logo-ui.png")) {
            write!(s, "{b:02x}").unwrap();
        }
        s
    };
    let html = include_str!("../assets/browser.html").replace("__LOGO_HEX__", &logo);
    diagnostic("Building UI WebView");
    let initial_ui = std::cell::Cell::new(true);
    let ui = WebViewBuilder::new_with_web_context(&mut private_context)
        .with_additional_browser_args("")
        .with_background_color(browser_background(store.settings.dark))
        .with_html(html)
        .with_bounds(bounds(0., 0., 1280., 840.))
        .with_devtools(cfg!(debug_assertions))
        .with_navigation_handler(move |u| {
            u == "about:blank" || (u.starts_with("data:text/html;") && initial_ui.replace(false))
        })
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_ipc_handler(move |request| {
            let _ = p.send_event(Msg::Ui(request.body().clone()));
        })
        .build_as_child(&window)?;
    diagnostic("UI WebView ready");
    shortcuts(&ui, proxy.clone());
    let session = if store.settings.restore || args.iter().any(|a| a == "--restore-after-update") {
        store.session.clone()
    } else {
        vec![]
    };
    let blocker = Rc::new(RefCell::new(adblock::Blocker::new(
        store.settings.adblock_enabled,
        &store.settings.adblock_allowed_sites,
    )));
    let downloads = downloads::Manager::new(
        store
            .downloads
            .iter()
            .map(|d| d.id)
            .max()
            .unwrap_or(0)
            .max(now() * 1000)
            + 1,
    );
    let mut app = Browser {
        content_fullscreen: None,
        update: updater::State::default(),
        update_available: None,
        update_plan: None,
        next_update_check: Instant::now() + Duration::from_secs(15),
        window,
        ui,
        tabs: vec![],
        active: 0,
        next: 1,
        split: None,
        split_ratio: 0.5,
        context,
        private_context,
        extension_host: None,
        extension_manager: None,
        extension_list: Vec::new(),
        extension_busy: false,
        extension_ready: false,
        extension_error: String::new(),
        store_prepared: None,
        extension_popup: None,
        extension_popup_id: String::new(),
        extension_popup_generation: 0,
        store,
        root,
        panel: String::new(),
        proxy,
        closed: vec![],
        exit: false,
        last_saved: None,
        last_ui_state: RefCell::new(Value::Null),
        next_maintenance: Instant::now() + Duration::from_secs(5),
        ui_bounds: None,
        ui_overlay: None,
        ui_overlay_radius: 0.,
        ui_region: None,
        blocker,
        downloads,
    };
    let inputs: Vec<_> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .take(32)
        .collect();
    if !inputs.is_empty() {
        for input in inputs {
            app.new_tab(input, args.iter().any(|a| a == "--private"))?;
        }
    } else if args.iter().any(|a| a == "--private") {
        app.new_tab("", true)?;
    } else if !session.is_empty() && !args.iter().any(|a| a == "--new-window") {
        for (index, u) in session.iter().take(30).enumerate() {
            app.active = app.add_deferred_tab(u, false);
            if let Some(tab) = app.tabs.last_mut() {
                tab.pinned = app.store.session_pins.get(index).copied().unwrap_or(false);
            }
        }
        app.tabs.sort_by_key(|t| !t.pinned);
        app.ensure_view(app.active)?;
        app.layout();
        app.sync();
    } else {
        app.new_tab("", false)?;
    }
    if let Ok(bytes) = fs::read(app.root.join("update-result.json")) {
        if let Ok(receipt) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(message) = receipt["message"].as_str() {
                app.update.message = message.into();
                if receipt["failed"] == true {
                    app.update.status = "error".into();
                }
            }
        }
        let _ = fs::remove_file(app.root.join("update-result.json"));
    }
    updater::cleanup(&app.root);
    app.sync();
    if !app.store.extensions.is_empty() {
        app.refresh_extensions();
    }
    diagnostic("Initial tab ready; running");
    event_loop.run(move |event, _, control_flow| {
        let _keep_instance = &instance;
        *control_flow = ControlFlow::WaitUntil(app.next_maintenance);
        match event {
            Event::UserEvent(msg) => {
                app.message(msg);
                if app.exit {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                diagnostic("Window close requested");
                app.save();
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                app.layout();
                app.sync();
            }
            _ => {}
        }
        if Instant::now() >= app.next_maintenance {
            app.maintain();
        }
        app.present_ready();
        if !matches!(*control_flow, ControlFlow::Exit) {
            let next = app
                .tabs
                .iter()
                .filter_map(|tab| tab.reveal_at)
                .fold(app.next_maintenance, Instant::min);
            *control_flow = ControlFlow::WaitUntil(next);
        }
    });
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(result) = updater::helper(&args) {
        if let Err(error) = result {
            diagnostic(&format!("Update failed: {error}"));
            let text: Vec<u16> = format!("Pembaruan Azy Core tidak selesai.\n\n{error}")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    None,
                    windows::core::PCWSTR(text.as_ptr()),
                    windows::core::w!("Pembaruan Azy Core"),
                    windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
                );
            }
        }
        return;
    }
    std::panic::set_hook(Box::new(|info| diagnostic(&format!("Panic: {info}"))));
    if let Err(e) = run() {
        diagnostic(&format!("Startup error: {e}"));
        let text = format!(
            "Azy Core gagal dijalankan. Pastikan Microsoft Edge WebView2 Runtime terpasang.\n\n{e}"
        );
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                windows::core::PCWSTR(wide.as_ptr()),
                windows::core::w!("Azy Core"),
                windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
}
