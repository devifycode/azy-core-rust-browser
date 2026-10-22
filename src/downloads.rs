use crate::Msg;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};
use tao::event_loop::EventLoopProxy;
use webview2_com::{
    DownloadStartingEventHandler, Microsoft::Web::WebView2::Win32::*, StateChangedEventHandler,
};
use windows::{
    core::{Interface, HSTRING},
    Win32::{
        Foundation::HWND,
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
        UI::Shell::{
            FileOpenDialog, IFileDialogCustomize, IFileOpenDialog, IShellItem,
            SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PICKFOLDERS,
            SIGDN_FILESYSPATH,
        },
    },
};

pub struct Pending {
    args: ICoreWebView2DownloadStartingEventArgs,
    deferral: ICoreWebView2Deferral,
    pub url: String,
    pub name: String,
    completed: bool,
}
impl Pending {
    pub fn start(
        &mut self,
        path: &Path,
        id: u64,
        proxy: EventLoopProxy<Msg>,
    ) -> windows::core::Result<()> {
        unsafe {
            self.args
                .SetResultFilePath(&HSTRING::from(path.as_os_str()))?;
            let mut token = 0;
            self.args.DownloadOperation()?.add_StateChanged(
                &StateChangedEventHandler::create(Box::new(move |operation, _| {
                    if let Some(operation) = operation {
                        let mut state = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
                        operation.State(&mut state)?;
                        if state != COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                            let _ = proxy.send_event(Msg::DownloadFinished(
                                id,
                                state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED,
                            ));
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            self.deferral.Complete()?;
            self.completed = true;
        }
        Ok(())
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        if !self.completed {
            unsafe {
                let _ = self.args.SetCancel(true);
                let _ = self.deferral.Complete();
            }
        }
    }
}

#[derive(Clone)]
pub struct Manager {
    next: Rc<Cell<u64>>,
    pending: Rc<RefCell<BTreeMap<u64, Pending>>>,
}
impl Manager {
    pub fn new(next: u64) -> Self {
        Self {
            next: Rc::new(Cell::new(next)),
            pending: Default::default(),
        }
    }
    pub fn take(&self, id: u64) -> Option<Pending> {
        self.pending.borrow_mut().remove(&id)
    }
    pub fn attach(
        &self,
        webview: &ICoreWebView2,
        proxy: EventLoopProxy<Msg>,
    ) -> windows::core::Result<()> {
        let manager = self.clone();
        unsafe {
            let mut token = 0;
            webview.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
                &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        crate::diagnostic("DownloadStarting received");
                        let mut path = windows::core::PWSTR::null();
                        args.ResultFilePath(&mut path)?;
                        let name = PathBuf::from(webview2_com::take_pwstr(path))
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        let mut url = windows::core::PWSTR::null();
                        args.DownloadOperation()?.Uri(&mut url)?;
                        let url = webview2_com::take_pwstr(url);
                        args.SetHandled(true)?;
                        let pending = Pending {
                            args: args.clone(),
                            deferral: args.GetDeferral()?,
                            url,
                            name,
                            completed: false,
                        };
                        let id = manager.next.get();
                        crate::diagnostic(&format!("DownloadStarting deferred {id}"));
                        manager.next.set(id + 1);
                        manager.pending.borrow_mut().insert(id, pending);
                        // Complete the WebView2 event first; show the modal from the app event loop.
                        if proxy.send_event(Msg::DownloadRequested(id)).is_err() {
                            let request = manager.take(id);
                            drop(request);
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }
        Ok(())
    }
}

pub fn choose_folder(
    initial: &Path,
    name: Option<&str>,
    remember: bool,
    owner: HWND,
) -> windows::core::Result<Option<(PathBuf, bool)>> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR,
        )?;
        dialog.SetTitle(&HSTRING::from("Pilih folder unduhan — Azy Core"))?;
        dialog.SetOkButtonLabel(&HSTRING::from("Pilih folder"))?;
        if let Ok(folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(
            &HSTRING::from(initial.as_os_str()),
            None,
        ) {
            dialog.SetFolder(&folder)?;
        }
        let controls = dialog.cast::<IFileDialogCustomize>()?;
        if let Some(name) = name {
            controls.AddText(7000, &HSTRING::from(format!("File: {}", safe_name(name))))?;
        }
        controls.AddCheckButton(
            7001,
            &HSTRING::from("Gunakan folder ini otomatis untuk unduhan berikutnya"),
            remember,
        )?;
        if let Err(error) = dialog.Show(Some(owner)) {
            if error.code().0 as u32 == 0x800704c7 {
                return Ok(None);
            }
            return Err(error);
        }
        let folder = PathBuf::from(webview2_com::take_pwstr(
            dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?,
        ));
        Ok(Some((
            folder,
            controls.GetCheckButtonState(7001)?.as_bool(),
        )))
    }
}

pub fn safe_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if name.is_empty()
        || matches!(
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
    {
        format!("download{name}")
    } else {
        name.into()
    }
}
pub fn target(folder: &Path, name: &str, reserved: &[PathBuf]) -> PathBuf {
    let name = safe_name(name);
    let path = folder.join(&name);
    let stem = Path::new(&name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let ext = Path::new(&name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut path = path;
    let mut n = 1;
    while path.exists()
        || reserved.iter().any(|p| {
            p.to_string_lossy()
                .eq_ignore_ascii_case(&path.to_string_lossy())
        })
    {
        path = folder.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    path
}
pub fn prepare_folder(folder: &Path, id: u64) -> std::io::Result<()> {
    if !folder.is_absolute() {
        return Err(std::io::Error::other("Pilih lokasi folder absolut."));
    }
    fs::create_dir_all(folder)?;
    let probe = folder.join(format!(
        ".azy-download-check-{}-{id}.tmp",
        std::process::id()
    ));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)?;
    drop(file);
    fs::remove_file(probe)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_reserved_targets_are_safe() {
        assert_eq!(safe_name("../a?.txt"), ".._a_.txt");
        assert_eq!(safe_name("CON.txt"), "downloadCON.txt");
        assert_eq!(safe_name(""), "download");
        let folder = Path::new("C:\\Azy Test");
        let reserved = vec![folder.join("report.txt"), folder.join("report (1).txt")];
        assert_eq!(
            target(folder, "report.txt", &reserved),
            folder.join("report (2).txt")
        );
    }
}
