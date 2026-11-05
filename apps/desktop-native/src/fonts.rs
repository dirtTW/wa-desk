//! The type stack: the bundled faces first, then the OS faces that carry
//! CJK, Korean and colour emoji (egui's own set is Latin-only).

use crate::app::diag;

/// Fallback faces the bundled set lacks, in fallback order: emoji, Chinese
/// (simplified *and* traditional), Japanese, then Korean. Before this, non-Latin
/// text drew as empty boxes.
/// Base-OS faces for symbol text that the Latin, emoji and CJK faces do not
/// carry: Mathematical Alphanumeric Symbols (U+1D400-U+1D7FF, e.g. "𝕋𝕒𝕦𝕗𝕚𝕢"),
/// arrows, box drawing, dingbat *letters* (U+2701-U+27BE without emoji
/// presentation). Segoe UI Symbol ships with stock Windows; the Linux and
/// macOS entries are the same coverage for other platforms, so a missing file
/// is simply skipped.
pub(crate) const SYMBOL_FONTS: &[&str] = &[
    "C:\\Windows\\Fonts\\seguisym.ttf",
    "C:\\Windows\\Fonts\\cambria.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMath-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/System/Library/Fonts/Apple Symbols.ttf",
];

/// Fallback faces the bundled set lacks, in fallback order: symbols, emoji,
/// Chinese (simplified *and* traditional), Japanese, then Korean. Before this,
/// non-Latin text drew as empty boxes.
pub(crate) fn install_fonts(ctx: &egui::Context) {
    // The colour-emoji face is *not* in this chain: bubbles and the picker
    // rasterise emoji with `emoji_glyph` (swash), which reads the same file
    // directly. Keeping a second 12 MB copy here only cost memory.
    let groups: [(&str, &[&str]); 4] = [
        // Symbols and mathematical letters *before* the emoji face: a chat
        // titled "𝕋𝕒𝕦𝕗𝕚𝕢" (U+1D54B ...) used to draw as six missing-glyph
        // boxes because no face in the chain carried U+1D400-U+1D7FF. Segoe UI
        // Symbol ships with every Windows 10/11 install.
        ("system-symbols", SYMBOL_FONTS),
        (
            "system-cjk",
            &[
                // Microsoft YaHei (GB18030: simplified, traditional, kana), then
                // Traditional-first faces, then the older SimSun.
                "C:\\Windows\\Fonts\\msyh.ttc",
                "C:\\Windows\\Fonts\\msyh.ttf",
                "C:\\Windows\\Fonts\\msjh.ttc",
                "C:\\Windows\\Fonts\\simsun.ttc",
                "C:\\Windows\\Fonts\\mingliu.ttc",
                // Linux / macOS.
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
                "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
                "/usr/share/fonts/truetype/arphic/uming.ttc",
                "/System/Library/Fonts/PingFang.ttc",
            ],
        ),
        (
            "system-japanese",
            &[
                // Kana and Japanese-only kanji the Chinese faces do not carry.
                "C:\\Windows\\Fonts\\YuGothM.ttc",
                "C:\\Windows\\Fonts\\YuGothR.ttc",
                "C:\\Windows\\Fonts\\meiryo.ttc",
                "C:\\Windows\\Fonts\\msgothic.ttc",
                "/usr/share/fonts/truetype/takao-gothic/TakaoPGothic.ttf",
                "/usr/share/fonts/truetype/vlgothic/VL-Gothic-Regular.ttf",
            ],
        ),
        (
            "system-hangul",
            &[
                // Hangul is not in the Chinese faces above.
                "C:\\Windows\\Fonts\\malgun.ttf",
                "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
            ],
        ),
    ];
    let mut fonts = egui::FontDefinitions::default();
    let mut added = false;
    for (key, candidates) in groups {
        let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) else {
            continue;
        };
        diag(&format!("font {key}: {} kB", bytes.len() / 1024));
        fonts.font_data.insert(
            key.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push(key.to_owned());
        }
        added = true;
    }
    if added {
        ctx.set_fonts(fonts);
    }
}
