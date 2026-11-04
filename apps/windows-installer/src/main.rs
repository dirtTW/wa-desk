//! WA Desk's installer stub.
//!
//! `pack-setup` glues this binary to the app and a trailer; running the result
//! installs the app and starts it (see the library docs).

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    if let Err(err) = wa_windows_installer::run() {
        wa_windows_installer::report_error(&err);
        std::process::exit(1);
    }
}
