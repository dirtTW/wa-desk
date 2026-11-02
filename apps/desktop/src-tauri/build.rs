use std::path::Path;

fn main() {
    // The UI folder is embedded through `tauri-build`, but the generated asset
    // builder only watches `frontendDist` when the tauri CLI tells it to.
    // Plain `cargo build` never sees frontend edits, which is why
    // `build-portable.bat` could ship a stale index.html ("Finished in 0.6s").
    // Force a rerun whenever any UI file changes.
    let ui_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui");
    println!("cargo:rerun-if-changed={}", ui_dir.display());
    let mut stack = vec![ui_dir];
    while let Some(cur) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&cur) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if entry.file_name().to_string_lossy() != "node_modules" {
                        stack.push(path);
                    }
                } else {
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
    }

    tauri_build::build()
}
