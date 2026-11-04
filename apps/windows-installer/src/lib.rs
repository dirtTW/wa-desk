//! WA Desk's Windows installer.
//!
//! The shipped `WA Desk Setup.exe` is two files glued together by
//! `pack-setup`: this stub, then the app binary, then a 16-byte trailer
//! (`WADSETUP` + payload length). Running it copies the app into
//! `%LOCALAPPDATA%\Programs\WA Desk`, creates Start Menu and Desktop
//! shortcuts, and starts it.
//!
//! Everything is plain Rust: the shortcuts go through `IShellLink` COM, so the
//! installer needs no external tool (the old MSI came from the Tauri bundler).

use std::path::{Path, PathBuf};

/// The installed executable's name.
pub const APP_NAME: &str = "WA Desk.exe";

/// Marks the trailer at the very end of a packed installer.
const MAGIC: &[u8; 8] = b"WADSETUP";

/// `MAGIC` (8 bytes) + payload length (8 bytes, little-endian).
const TRAILER: usize = 16;

/// The app bytes carried by a packed installer, if this binary has a payload.
pub fn payload(all: &[u8]) -> Option<&[u8]> {
    if all.len() < TRAILER {
        return None;
    }
    let trailer = &all[all.len() - TRAILER..];
    if &trailer[..8] != MAGIC {
        return None;
    }
    let length = u64::from_le_bytes(trailer[8..16].try_into().ok()?) as usize;
    let start = all.len().checked_sub(TRAILER.checked_add(length)?)?;
    Some(&all[start..all.len() - TRAILER])
}

/// Build a packed installer from the stub and the app binary.
pub fn pack(stub: &[u8], app: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(stub.len() + app.len() + TRAILER);
    out.extend_from_slice(stub);
    out.extend_from_slice(app);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(app.len() as u64).to_le_bytes());
    out
}

/// `%LOCALAPPDATA%\Programs\WA Desk` - a per-user install needs no admin.
pub fn install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Programs").join("WA Desk")
}

/// Run the installer: find the payload, install it, start the app.
pub fn run() -> Result<(), String> {
    #[cfg(windows)]
    {
        run_windows()
    }
    #[cfg(not(windows))]
    {
        Err("the WA Desk installer is Windows-only".to_string())
    }
}

#[cfg(windows)]
fn run_windows() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| format!("where am I? {err}"))?;
    let bytes =
        std::fs::read(&exe).map_err(|err| format!("could not read the installer: {err}"))?;
    let app = match payload(&bytes) {
        Some(payload) => payload.to_vec(),
        None => {
            // A development stub sits next to the app binary.
            let sibling = exe.with_file_name("wa-desk-native.exe");
            std::fs::read(&sibling).map_err(|_| {
                "this installer has no payload - build it with `pack-setup`".to_string()
            })?
        }
    };

    let target = install(&app)?;
    let _ = std::process::Command::new(&target).spawn();
    Ok(())
}

/// Write the app, its uninstall helper and the shortcuts. Returns the installed
/// executable.
pub fn install(app: &[u8]) -> Result<PathBuf, String> {
    let dir = install_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("could not create {}: {err}", dir.display()))?;

    let target = dir.join(APP_NAME);
    let staging = dir.join("WA Desk.new.exe");
    std::fs::write(&staging, app)
        .map_err(|err| format!("could not write {}: {err}", staging.display()))?;

    // A running copy can be renamed on Windows, just not overwritten.
    if target.exists() {
        let old = dir.join("WA Desk.old.exe");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&target, &old)
            .map_err(|err| format!("could not set the old app aside: {err}"))?;
    }
    std::fs::rename(&staging, &target)
        .map_err(|err| format!("could not put the app in place: {err}"))?;

    // The app folder is the uninstall: a helper that removes it.
    let uninstall = dir.join("Uninstall WA Desk.cmd");
    let _ = std::fs::write(
        &uninstall,
        format!(
            "@echo off\r\ntaskkill /im \"{APP_NAME}\" >nul 2>&1\r\nrd /s /q \"{}\"\r\n",
            dir.display()
        ),
    );

    shortcuts(&target);
    Ok(target)
}

/// Start Menu and Desktop shortcuts, best effort.
#[cfg(windows)]
fn shortcuts(target: &Path) {
    if let Some(programs) = std::env::var_os("APPDATA").map(|dir| {
        PathBuf::from(dir)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
    }) {
        let _ = std::fs::create_dir_all(&programs);
        let _ = link(&programs.join("WA Desk.lnk"), target);
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let desktop = PathBuf::from(profile).join("Desktop");
        if desktop.is_dir() {
            let _ = link(&desktop.join("WA Desk.lnk"), target);
        }
    }
}

#[cfg(not(windows))]
fn shortcuts(_target: &Path) {}

/// One `.lnk` through `IShellLink` + `IPersistFile`.
#[cfg(windows)]
fn link(link_path: &Path, target: &Path) -> Result<(), String> {
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        IPersistFile,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface};

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|err| err.to_string())?;
        let exe = HSTRING::from(target.as_os_str());
        shell.SetPath(&exe).map_err(|err| err.to_string())?;
        if let Some(dir) = target.parent() {
            shell
                .SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))
                .map_err(|err| err.to_string())?;
        }
        shell
            .SetIconLocation(&exe, 0)
            .map_err(|err| err.to_string())?;
        let file: IPersistFile = shell.cast().map_err(|err| err.to_string())?;
        file.Save(&HSTRING::from(link_path.as_os_str()), true)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Show an installer error without a console.
pub fn report_error(message: &str) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        use windows::core::HSTRING;
        unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(message),
                &HSTRING::from("WA Desk installer"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    eprintln!("WA Desk installer: {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_payload_round_trips() {
        let packed = pack(b"the stub", b"the app");
        assert_eq!(payload(&packed), Some(&b"the app"[..]));
    }

    #[test]
    fn a_plain_binary_has_no_payload() {
        assert!(payload(b"").is_none());
        assert!(payload(b"just a binary").is_none());
    }

    #[test]
    fn a_truncated_trailer_is_rejected() {
        let mut packed = pack(b"stub", b"app");
        packed.truncate(packed.len() - 3);
        assert!(payload(&packed).is_none());
        // A trailer that claims more bytes than the file holds is rejected too.
        let mut lying = b"stub".to_vec();
        lying.extend_from_slice(MAGIC);
        lying.extend_from_slice(&(9999_u64).to_le_bytes());
        assert!(payload(&lying).is_none());
    }

    #[test]
    fn the_install_dir_is_per_user() {
        let dir = install_dir();
        assert!(dir.ends_with(Path::new("Programs").join("WA Desk")));
        assert!(dir.join(APP_NAME).ends_with(APP_NAME));
    }
}
