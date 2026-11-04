//! Embeds `assets/icon.ico` in the Windows executable, so Explorer, the taskbar
//! and shortcuts show the WA Desk icon instead of the generic one. Linux shows
//! the window icon from `src/icon.rs`.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "WA Desk");
        resource.set("FileDescription", "WA Desk");
        resource.set("LegalCopyright", "MIT");
        if let Err(err) = resource.compile() {
            // The rc tool comes with the MSVC build tools; a missing one must
            // not break the build, the app just keeps the default icon.
            println!("cargo:warning=could not embed the exe icon: {err}");
        }
    }
}
