//! The Windows system tray: the WA Desk icon with an "Open WA Desk" / "Quit"
//! menu, drawn with Win32 directly (`Shell_NotifyIcon`) so the build does not
//! pull GTK/glib in through a cross-platform tray crate. Linux has no tray; the
//! module compiles to a no-op there so the rest of the window is unchanged.

/// What the user asked for from the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum TrayAction {
    Open,
    Quit,
}

/// Map a menu id back to an action (kept pure so it can be unit tested).
#[cfg_attr(not(test), allow(dead_code))]
pub fn action_for(id: &str) -> Option<TrayAction> {
    match id {
        "open" => Some(TrayAction::Open),
        "quit" => Some(TrayAction::Quit),
        _ => None,
    }
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicU8, Ordering};

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPV5HEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
        DeleteObject, GetDC, ReleaseDC,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW,
        HICON, HMENU, ICONINFO, MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage,
        RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        TrackPopupMenu, TranslateMessage, WM_APP, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY,
        WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
    };

    /// The tray callback message (`WM_APP + 1`).
    const CALLBACK: u32 = WM_APP + 1;
    /// Nothing, open, quit.
    static ACTION: AtomicU8 = AtomicU8::new(0);

    const CMD_OPEN: usize = 1;
    const CMD_QUIT: usize = 2;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            CALLBACK => {
                let event = (lparam as u32) & 0xFFFF;
                if event == WM_LBUTTONUP {
                    ACTION.store(1, Ordering::Relaxed);
                } else if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    unsafe { show_menu(hwnd) };
                }
                0
            }
            WM_COMMAND => {
                let command = wparam & 0xFFFF;
                if command == CMD_OPEN {
                    ACTION.store(1, Ordering::Relaxed);
                } else if command == CMD_QUIT {
                    ACTION.store(2, Ordering::Relaxed);
                }
                0
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    /// Pop the menu up at the cursor and record what was chosen.
    unsafe fn show_menu(hwnd: HWND) {
        let menu: HMENU = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return;
        }
        let open = wide("Open WA Desk");
        let quit = wide("Quit WA Desk");
        unsafe {
            AppendMenuW(menu, MF_STRING, CMD_OPEN, open.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, CMD_QUIT, quit.as_ptr());
        }
        let mut point = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut point);
            // Without the foreground window the menu would not dismiss when the
            // user clicks elsewhere - a documented Win32 quirk.
            SetForegroundWindow(hwnd);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                hwnd,
                std::ptr::null(),
            ) as usize;
            PostMessageW(hwnd, WM_NULL, 0, 0);
            DestroyMenu(menu);
            if command == CMD_OPEN {
                ACTION.store(1, Ordering::Relaxed);
            } else if command == CMD_QUIT {
                ACTION.store(2, Ordering::Relaxed);
            }
        }
    }

    /// Build an `HICON` from the window icon's RGBA pixels.
    unsafe fn icon_from_rgba(rgba: &[u8], width: u32, height: u32) -> HICON {
        if width == 0 || height == 0 || rgba.len() < (width * height * 4) as usize {
            return std::ptr::null_mut();
        }
        unsafe {
            let mut header: BITMAPV5HEADER = std::mem::zeroed();
            header.bV5Size = std::mem::size_of::<BITMAPV5HEADER>() as u32;
            header.bV5Width = width as i32;
            header.bV5Height = -(height as i32); // top-down rows
            header.bV5Planes = 1;
            header.bV5BitCount = 32;
            header.bV5Compression = BI_RGB;
            header.bV5RedMask = 0x00FF_0000;
            header.bV5GreenMask = 0x0000_FF00;
            header.bV5BlueMask = 0x0000_00FF;
            header.bV5AlphaMask = 0xFF00_0000;

            let screen = GetDC(std::ptr::null_mut());
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let colour = CreateDIBSection(
                screen,
                &header as *const BITMAPV5HEADER as *const BITMAPINFO,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            ReleaseDC(std::ptr::null_mut(), screen);
            if colour.is_null() || bits.is_null() {
                return std::ptr::null_mut();
            }
            // Windows wants BGRA, top-down.
            let target = std::slice::from_raw_parts_mut(bits as *mut u8, rgba.len());
            for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
                target[index * 4] = pixel[2];
                target[index * 4 + 1] = pixel[1];
                target[index * 4 + 2] = pixel[0];
                target[index * 4 + 3] = pixel[3];
            }
            // An all-zero mask keeps every pixel visible.
            let mask = CreateBitmap(width as i32, height as i32, 1, 1, std::ptr::null());
            let info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: colour,
            };
            let icon = CreateIconIndirect(&info);
            DeleteObject(colour as _);
            DeleteObject(mask as _);
            icon
        }
    }

    /// Install the icon and the message window; returns the window handle.
    pub fn install(rgba: &[u8], width: u32, height: u32) -> Option<isize> {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class_name = wide("WADeskTrayWindow");
            let mut class: WNDCLASSW = std::mem::zeroed();
            class.lpfnWndProc = Some(window_proc);
            class.hInstance = instance;
            class.lpszClassName = class_name.as_ptr();
            if RegisterClassW(&class) == 0 {
                // Already registered by an earlier call is fine.
            }
            let title = wide("WA Desk");
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return None;
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, hwnd as isize);

            let icon = icon_from_rgba(rgba, width, height);
            if icon.is_null() {
                DestroyWindow(hwnd);
                return None;
            }
            let mut data: NOTIFYICONDATAW = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = 1;
            data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            data.uCallbackMessage = CALLBACK;
            data.hIcon = icon;
            let tip = wide("WA Desk");
            let count = tip.len().min(data.szTip.len());
            data.szTip[..count].copy_from_slice(&tip[..count]);
            if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
                DestroyWindow(hwnd);
                return None;
            }
            Some(hwnd as isize)
        }
    }

    /// Drop the icon and the window.
    pub fn remove(hwnd: isize) {
        unsafe {
            let hwnd = hwnd as HWND;
            let mut data: NOTIFYICONDATAW = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &data);
            DestroyWindow(hwnd);
        }
    }

    /// Take the action the user picked since the last call.
    pub fn take() -> Option<super::TrayAction> {
        match ACTION.swap(0, Ordering::Relaxed) {
            1 => Some(super::TrayAction::Open),
            2 => Some(super::TrayAction::Quit),
            _ => None,
        }
    }

    /// A hidden thread that pumps the tray window's messages. The tray needs a
    /// message loop, and the egui window already runs on the main thread.
    pub fn pump() {
        std::thread::spawn(|| unsafe {
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        });
    }
}

/// The installed tray icon.
#[cfg(windows)]
pub struct Tray {
    handle: isize,
}

#[cfg(windows)]
impl Tray {
    /// Install the tray icon. Returns `None` when the shell refuses it (a
    /// headless session, for example) - the app keeps working without one.
    pub fn new(icon: &egui::IconData) -> Option<Self> {
        let handle = win::install(&icon.rgba, icon.width, icon.height)?;
        win::pump();
        crate::app::diag("tray: icon installed");
        Some(Self { handle })
    }

    /// The action the user picked since the last frame, if any.
    pub fn poll(&self) -> Option<TrayAction> {
        win::take()
    }
}

#[cfg(windows)]
impl Drop for Tray {
    fn drop(&mut self) {
        win::remove(self.handle);
    }
}

#[cfg(not(windows))]
pub struct Tray;

#[cfg(not(windows))]
impl Tray {
    pub fn new(_icon: &egui::IconData) -> Option<Self> {
        None
    }

    pub fn poll(&self) -> Option<TrayAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_actions_are_distinct() {
        assert_ne!(TrayAction::Open, TrayAction::Quit);
    }

    #[test]
    fn menu_ids_map_to_actions() {
        assert_eq!(action_for("open"), Some(TrayAction::Open));
        assert_eq!(action_for("quit"), Some(TrayAction::Quit));
        assert_eq!(action_for("something-else"), None);
    }
}
