//! Reading files off the OS clipboard, for Ctrl+V.
//!
//! egui's clipboard support is text only; on Windows a file copy is a
//! `CF_HDROP` handle, read here with the Win32 clipboard API.

#[cfg(windows)]
pub fn first_file() -> Option<std::path::PathBuf> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    const CF_HDROP: u32 = 15;

    // SAFETY: the clipboard is opened/closed around every handle use, and the
    // returned paths are copied into owned buffers before it is closed.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let mut result = None;
        if IsClipboardFormatAvailable(CF_HDROP) != 0 {
            let handle = GetClipboardData(CF_HDROP);
            if !handle.is_null() {
                let count = DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0);
                if count > 0 {
                    let length = DragQueryFileW(handle, 0, std::ptr::null_mut(), 0);
                    if length > 0 {
                        let mut buffer = vec![0u16; length as usize + 1];
                        let written =
                            DragQueryFileW(handle, 0, buffer.as_mut_ptr(), buffer.len() as u32);
                        if written > 0 {
                            result = Some(std::path::PathBuf::from(String::from_utf16_lossy(
                                &buffer[..written as usize],
                            )));
                        }
                    }
                }
            }
        }
        CloseClipboard();
        result
    }
}

#[cfg(not(windows))]
pub fn first_file() -> Option<std::path::PathBuf> {
    None
}
