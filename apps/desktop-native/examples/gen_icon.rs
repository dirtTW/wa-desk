//! Generates the app icon (`assets/icon.png` + `assets/icon.ico`) from one SVG
//! description: a WhatsApp-green rounded square with a white speech bubble and
//! three green dots. Run it after changing the artwork:
//!
//! ```bash
//! cargo run -p wa-desktop-native --example gen_icon
//! ```

use std::path::PathBuf;

const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn art() -> String {
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">
  <rect width="256" height="256" rx="58" fill="#21C063"/>
  <path d="M128 44c-41.4 0-75 27.6-75 61.7 0 19.7 11.3 37.2 28.8 48.6l-6.7 26.4 30.2-14.2c7.3 1.9 15.1 2.9 22.7 2.9 41.4 0 75-27.6 75-61.7S169.4 44 128 44z" fill="#FFFFFF"/>
  <circle cx="99" cy="106" r="9" fill="#21C063"/>
  <circle cx="128" cy="106" r="9" fill="#21C063"/>
  <circle cx="157" cy="106" r="9" fill="#21C063"/>
</svg>"##
        .to_string()
}

/// Render the artwork at `size`, using the SVG's own 256-unit viewBox.
fn render(size: u32) -> Vec<u8> {
    let mut svg = art();
    svg = svg.replacen(
        "width=\"256\" height=\"256\"",
        &format!("width=\"{size}\" height=\"{size}\""),
        1,
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .expect("valid icon svg");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("pixmap");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().expect("png")
}

/// A PNG-compressed ICO (Vista and newer accept PNG payloads).
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in images {
        let edge = if *size >= 256 { 0u8 } else { *size as u8 };
        out.push(edge); // width
        out.push(edge); // height
        out.push(0); // palette
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    out
}

fn main() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    std::fs::create_dir_all(&assets).expect("assets dir");

    let largest = render(256);
    let png = assets.join("icon.png");
    std::fs::write(&png, &largest).expect("icon.png");

    let images: Vec<(u32, Vec<u8>)> = SIZES.iter().map(|size| (*size, render(*size))).collect();
    let ico_path = assets.join("icon.ico");
    std::fs::write(&ico_path, ico(&images)).expect("icon.ico");

    println!(
        "wrote {} ({} bytes) and {} ({} bytes)",
        png.display(),
        largest.len(),
        ico_path.display(),
        std::fs::metadata(&ico_path)
            .map(|meta| meta.len())
            .unwrap_or(0)
    );
}
