//! Launch at login, through the per-user Run key (Windows).
//!
//! Kept tiny and self-contained: the registry value is the absolute path of the
//! running executable, so an updated binary keeps starting.

#[cfg(windows)]
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
#[cfg(windows)]
const VALUE: &str = "WA Desk";

/// Whether the app currently starts with Windows.
pub fn enabled() -> bool {
    #[cfg(windows)]
    {
        registry_read().is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Turn launch-at-login on or off.
pub fn set(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        if enabled {
            let exe = std::env::current_exe().map_err(|err| err.to_string())?;
            registry_write(&exe.to_string_lossy())
        } else {
            registry_delete()
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("launch at login is only wired up on Windows".to_string())
    }
}

#[cfg(windows)]
fn registry_read() -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_READ, REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };

    let mut key = std::ptr::null_mut();
    let path = wide(RUN_KEY);
    // SAFETY: the key handle is closed before returning; the buffer is sized by
    // the query call below.
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let mut kind = 0u32;
        let mut size = 0u32;
        let name = wide(VALUE);
        let status = RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        );
        if status != 0 || kind != REG_SZ || size == 0 {
            RegCloseKey(key);
            return None;
        }
        let mut buffer = vec![0u8; size as usize];
        let status = RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            buffer.as_mut_ptr(),
            &mut size,
        );
        RegCloseKey(key);
        if status != 0 {
            return None;
        }
        let text: Vec<u16> = buffer
            .chunks(2)
            .filter(|pair| pair.len() == 2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|value| *value != 0)
            .collect();
        Some(String::from_utf16_lossy(&text))
    }
}

#[cfg(windows)]
fn registry_write(value: &str) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegOpenKeyExW, RegSetValueExW,
    };

    let mut key = std::ptr::null_mut();
    let path = wide(RUN_KEY);
    // SAFETY: all pointers come from wide() or local buffers that outlive the
    // calls; the key is closed on every path.
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key) != 0 {
            return Err("could not open the Run key".to_string());
        }
        let data = wide(value);
        let bytes = data.len() * 2;
        let status = RegSetValueExW(
            key,
            wide(VALUE).as_ptr(),
            0,
            REG_SZ,
            data.as_ptr().cast(),
            bytes as u32,
        );
        RegCloseKey(key);
        if status != 0 {
            return Err("could not write the Run value".to_string());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn registry_delete() -> Result<(), String> {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_SET_VALUE, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    };

    let mut key = std::ptr::null_mut();
    let path = wide(RUN_KEY);
    // SAFETY: same contract as above.
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key) != 0 {
            return Ok(());
        }
        let status = RegDeleteValueW(key, wide(VALUE).as_ptr());
        RegCloseKey(key);
        if status != 0 {
            return Err("could not remove the Run value".to_string());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn the_run_key_round_trips() {
        // Never touches the machine's real setting: it reads and restores it.
        let before = super::enabled();
        let original = super::registry_read();
        super::set(true).expect("enable");
        assert!(super::enabled());
        super::set(false).expect("disable");
        assert!(!super::enabled());
        if let Some(previous) = original {
            super::registry_write(&previous).expect("restore");
        } else {
            let _ = before;
        }
    }
}
