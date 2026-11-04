//! Packs the app binary into the installer stub.
//!
//! ```text
//! cargo run -p wa-windows-installer --bin pack-setup -- \
//!     target/release/wa-desk-setup.exe \
//!     target/release/wa-desk-native.exe \
//!     "dist/WA Desk Setup.exe"
//! ```

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let [stub, app, out] = args.as_slice() else {
        eprintln!("usage: pack-setup <stub.exe> <app.exe> <out.exe>");
        std::process::exit(2);
    };
    if let Err(err) = pack(stub, app, out) {
        eprintln!("pack-setup: {err}");
        std::process::exit(1);
    }
}

fn pack(stub: &Path, app: &Path, out: &Path) -> Result<(), String> {
    let stub_bytes =
        std::fs::read(stub).map_err(|err| format!("could not read {}: {err}", stub.display()))?;
    let app_bytes =
        std::fs::read(app).map_err(|err| format!("could not read {}: {err}", app.display()))?;

    let packed = wa_windows_installer::pack(&stub_bytes, &app_bytes);
    // Never ship an installer whose payload cannot be found again.
    match wa_windows_installer::payload(&packed) {
        Some(found) if found == app_bytes.as_slice() => {}
        _ => return Err("the packed installer does not round-trip".to_string()),
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(out, &packed)
        .map_err(|err| format!("could not write {}: {err}", out.display()))?;
    println!(
        "wrote {} ({} stub bytes + {} app bytes)",
        out.display(),
        stub_bytes.len(),
        app_bytes.len()
    );
    Ok(())
}
