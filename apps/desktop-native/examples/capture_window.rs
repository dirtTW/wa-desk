//! Capture a top-level window to a PNG (Windows GDI), used to compare the
//! native window with the official client side by side.
//!
//! ```text
//! cargo run -p wa-desktop-native --example capture_window -- WhatsApp shot.png
//! cargo run -p wa-desktop-native --example capture_window -- list
//! ```

#[cfg(windows)]
fn main() {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows_sys::Win32::Storage::Xps::PrintWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextW, IsWindowVisible, SW_RESTORE, ShowWindow,
    };

    const PW_RENDERFULLCONTENT: u32 = 2;

    struct Find {
        needle: String,
        found: HWND,
        width: i32,
        height: i32,
    }

    unsafe extern "system" fn callback(window: HWND, param: LPARAM) -> i32 {
        unsafe {
            let find = &mut *(param as *mut Find);
            if IsWindowVisible(window) == 0 {
                return 1;
            }
            let mut buffer = [0u16; 512];
            let length = GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32);
            if length <= 0 {
                return 1;
            }
            let title = String::from_utf16_lossy(&buffer[..length as usize]);
            if find.needle == "list" {
                println!("window: {title}");
                return 1;
            }
            if !title.contains(&find.needle) {
                return 1;
            }
            let mut rect: RECT = std::mem::zeroed();
            // A minimized window reports a tiny off-screen rect; restore it.
            let _ = ShowWindow(window, SW_RESTORE);
            std::thread::sleep(std::time::Duration::from_millis(300));
            if GetWindowRect(window, &mut rect) == 0 {
                return 1;
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if width < 200 || height < 200 {
                return 1;
            }
            println!("found '{title}' {width}x{height}");
            find.found = window;
            find.width = width;
            find.height = height;
            0
        }
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let needle = args.first().cloned().unwrap_or_else(|| "WhatsApp".into());
    let output = args.get(1).cloned().unwrap_or_else(|| "capture.png".into());

    let mut find = Find {
        needle,
        found: null_mut(),
        width: 0,
        height: 0,
    };
    unsafe {
        EnumWindows(Some(callback), &mut find as *mut Find as LPARAM);
        if find.found.is_null() {
            eprintln!("no window matched");
            std::process::exit(1);
        }

        let screen = GetDC(find.found);
        let memory = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, find.width, find.height);
        let previous = SelectObject(memory, bitmap);
        if PrintWindow(find.found, memory, PW_RENDERFULLCONTENT) == 0 {
            // Some windows refuse the full-content flag; try the classic path.
            PrintWindow(find.found, memory, 0);
        }

        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = find.width;
        info.bmiHeader.biHeight = -find.height; // top-down
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;

        let mut pixels = vec![0u8; (find.width * find.height * 4) as usize];
        let lines = GetDIBits(
            memory,
            bitmap,
            0,
            find.height as u32,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(memory, previous);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(find.found, screen);

        if lines == 0 {
            eprintln!("GetDIBits failed");
            std::process::exit(1);
        }
        // BGRA -> RGBA.
        for pixel in pixels.chunks_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let image = image::RgbaImage::from_raw(find.width as u32, find.height as u32, pixels)
            .expect("image size");
        image.save(&output).expect("save");
        println!("wrote {output}");
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("capture_window is Windows-only");
}
