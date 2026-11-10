# WA Desk

An unofficial WhatsApp desktop client in **Rust** for Windows and Linux. It speaks the WhatsApp Web
multi-device protocol through [`whatsapp-rust`](https://crates.io/crates/whatsapp-rust) and draws its
window with `eframe`/`egui` — **no WebView, no HTML, no JavaScript anywhere in the repository**.

> **Disclaimer.** Not affiliated with, endorsed by, or connected to WhatsApp or Meta. A custom client
> can violate Meta's Terms of Service and can get the linked account **banned**. Use a secondary
> account if you care about the primary one.

## Development note

**This project is 100% vibe-coded — the author does not write code.** I described what I wanted, looked
at the result and said "no — like *this*" until it worked.

**Built with:** [DeepSeek V4.1 Flash](https://deepseek.com) (primary) and
[GLM 5.3 Flash](https://z.ai), inside [OpenCode](https://opencode.ai).

It works, but it has **not had a security audit**. It handles end-to-end encrypted messages and stores
credentials: read it before trusting it with anything sensitive, and use a secondary account. Issues and
pull requests are welcome.

## What it does

- **Linking** — QR or phone-number pairing code, persistent SQLite session, live connection state.
- **Chat list like the phone** — rail with unread/unseen badges and your photo, kebab, green new-chat
  button, rounded search, and the live filter row (**All / Unread / Favorites / Communities / Groups**
  plus a round `+` for new lists). Rows show real avatars, previews, clocks, unread pills, **pin** and
  **muted** badges; the list is cached to `chats.json` and backfilled in batches.
- **Conversation** — text, typing, tails, sender colours, inline clocks, **albums** (a burst of
  photos/videos paints as one grid bubble), **view-once** placeholders, documents as the official card,
  link previews, replies, mentions, reactions; in-chat search with counter and highlight.
- **Media** — photos, videos and voice notes. Videos pre-decode through `ffmpeg` and play with the
  official transport bar (play/pause, drag seek, speed, mute, fullscreen); a file that has not arrived
  shows a **download disc** instead of a dead tile. The full-screen viewer has the official toolbar
  (zoom, reply, star, collapse, download, forward, menu, close), scroll zoom, arrow paging, a centred
  caption and a filmstrip with duration badges.
- **Voice notes** — official player bubble with waveform, seek and elapsed/total (OGG/Opus via libopus).
- **Emoji and stickers** — full 2,300-glyph catalogue with search/recents, colour glyphs rasterised with
  `swash`, favourites and cached stickers, the official picker (tabs, store search, Create tile,
  Emoji/GIF/Stickers footer); animated WebP stickers animate, GIFs loop inline.
- **Message and chat actions** — reply, react, copy, forward, edit, delete (me/everyone), star; mute,
  pin, mark unread, archive, clear, delete; starred screen; create group, add/edit members, leave.
- **Status / stories and channels** — 24 h stories with progress bars, reply/react/delete and posting;
  channels get their own panel with read-only posts.
- **Groups and communities** — group info in the live shape (creator card, description, media strip,
  flat icon rows, member list), community drawer.
- **Calls** — 1:1 voice and video: start/accept/decline, mute, hang up; 16 kHz capture, peer audio
  playout, H.264 decode, own camera with a mirrored self-view, NLMS echo cancellation.
- **AI assistant** — bring your own OpenAI-compatible key (AES-256-GCM at rest), on/off and `/ai`
  toggles, casual Indonesian "gw/lu" persona, replies to saved contacts and to groups that @mention or
  quote you, typing indicator, human-like delays, bounded chat context, occasional reactions/stickers.
- **Desktop integration** — tray menu, batched toasts (one per chat burst), launch at login, native file
  picker, clipboard image paste, **in-app updater** with a progress bar on the button.
- **Looks like the official client** — the `--WDS-*` palette, doodle wallpaper, Material Symbols
  rasterised with `resvg`, measured bubbles, day chips, group gutters, and OS font fallbacks so CJK,
  Korean and nicknames like `𝕋𝕒𝕦𝕗𝕚𝕢` render properly.

**History caveat.** WhatsApp only pushes older history to a newly linked device. The app asks for recent
chats in batches and on open; Settings → Chat history can re-run it, and a WhatsApp “Export chat” `.txt`
can be imported into an open chat.

## How it is put together

```
crates/wa-types/          serializable vocabulary shared by protocol and UI
crates/wa-core/           the session: protocol, Signal, media, history, groups, status, calls
crates/wa-ui/             every UI decision: strings, labels, plans, timings (+ tests)
apps/desktop-native/      the window: eframe/egui, driven by UiEvent and Command
apps/windows-installer/   the setup stub that installs the app
```

`wa-core` knows nothing about the UI and `wa-ui` knows nothing about the session, so either can be reused
alone. The UI is verified without a display:

```bash
cargo run -p wa-desktop-native -- --selftest
# {"channels":1,"chats":3,"delete":"Delete message?","delete_everyone":true,"emoji":160,
#  "first":"Team","forward":1,"group_members":2,"linked":true,"photo_body":"Photo",
#  "preview":"ok","reactions":2,"reply":false,"search":"1/1","stamp":"9:40 am",
#  "status":"Connected","status_groups":2,"stickers":0,"theme":"light","viewer_items":1}
```

## Install on Windows

Download **`WA-Desk-Setup.exe`** from the [releases](https://github.com/dirtTW/wa-desk/releases) and run
it — no admin, no MSI/WiX. It installs to `%LOCALAPPDATA%\Programs\WA Desk`, adds Start Menu and Desktop
shortcuts, starts the app and leaves `Uninstall WA Desk.cmd` behind. Linux ships the plain
`wa-desk-native` binary.

## Requirements

- Rust **1.95+** (edition 2024) — [rustup](https://rustup.rs/).
- **Windows**: MSVC build tools ("Desktop development with C++").
- **Linux** (Debian/Ubuntu): `sudo apt install build-essential libssl-dev pkg-config libasound2-dev
  libxkbcommon-dev libwayland-dev libgl1-mesa-dev` (ALSA is needed for audio). Fedora/Arch:
  `openssl-devel`, `alsa-lib-devel`, `libxkbcommon-devel`, `wayland-devel`, `mesa-libGL-devel`.
- **`ffmpeg`** (optional; on `PATH`, next to the exe, or `WA_DESK_FFMPEG`) — video playback, animated
  stickers, the AI's video frames, camera capture and H.264 for calls. Without it clips keep the poster.

## Run and build

```bash
cargo run -p wa-desktop-native                                     # the app
cargo run -p wa-desktop-native -- --selftest                       # headless UI check
cargo run -p wa-desktop-native -- --preview --screenshot ui.png    # sample chat, no account

cargo build --release -p wa-desktop-native                         # target/release/wa-desk-native
cargo run -p wa-windows-installer --bin pack-setup -- \
  target/release/wa-desk-setup.exe target/release/wa-desk-native.exe "WA-Desk-Setup.exe"
```

First launch shows a QR code: on the phone open **WhatsApp → Settings → Linked devices → Link a device**,
or use *Link with phone number instead*. Build each target on its own OS. The release profile favours
size (fat LTO, one codegen unit, stripped, `panic = "abort"` → ~25 MB); CI uses `--profile ci` so lint and
test runs stay fast. `[profile.dev] incremental = false` keeps the debug cache from growing to tens of GB.

## Where data lives

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\io.github.dirttw.wadesk\whatsapp.db` |
| Linux | `~/.local/share/io.github.dirttw.wadesk/whatsapp.db` |

`chats.json`, `contacts.json`, `media.json`, `settings.json`, `ai.key` and `diagnostics.log` live in the
same folder, and downloaded media in `%APPDATA%\io.github.dirttw.wadesk-native\media`. Delete the database
to unlink locally (remove the device from the phone too).

## Versioning

Releases are pre-releases: `0.2.1-rc.4` is the fourth release candidate of `0.2.1`, and the counter rolls
at 99. Every [`CHANGELOG.md`](CHANGELOG.md) entry names the mechanism and how it was verified (tests,
screenshots, `--selftest`). The updater compares the numeric core first, then the `rc.N` number, so
`0.2.2-rc.1` is offered over `0.2.1-rc.9`.

## Credits

WA Desk is a thin layer on other people's work. Every project below is used by the shipped client; thank
you to all of them.

**Protocol and crypto**
- **[whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust)** by
  [jlucaso1](https://github.com/jlucaso1) — the multi-device client this app is built on:
  `whatsapp-rust` (patched in `vendor/whatsapp-rust`), `wacore`, `waproto`, `wacore-noise`,
  `wacore-appstate` (patched in `vendor/wacore-appstate`), `wacore-binary`, `wacore-libsignal` and the
  `whatsapp-rust-*` helpers.
- **[Signal](https://signal.org/) / [libsignal](https://github.com/signalapp/libsignal)** — Double
  Ratchet, X3DH and the primitives behind the end-to-end encryption; also why this app is AGPL.
- **[whatsmeow](https://github.com/tulir/whatsmeow)** and
  **[Baileys](https://github.com/WhiskeySockets/Baileys)** — the reference implementations
  `whatsapp-rust` checks itself against.
- **[SQLite](https://sqlite.org/)** through `rusqlite` — where the linked session lives.

**Window, text and pictures**
- **[eframe](https://github.com/emilk/egui) / [egui](https://github.com/emilk/egui)** by
  [emilk](https://github.com/emilk) — the native window and everything painted in it, plus `epaint`
  and `ecolor`.
- **[resvg](https://github.com/linebender/resvg) / [tiny-skia](https://github.com/linebender/tiny-skia)**
  — rasterising the pairing QR code and the Material icons.
- **[swash](https://github.com/dfrg/swash)** — turns colour-emoji glyphs (COLR/CPAL, CBDT) into RGBA
  textures.
- **[image](https://github.com/image-rs/image)** — decoding photos, posters, GIFs and stickers.
- **[Material Symbols](https://fonts.google.com/icons)** by Google — every icon, generated into
  `crates/wa-ui/src/icons_data.rs` (`cargo run -p wa-ui --example gen_icons`).
- **Unicode emoji** and the OS colour emoji face (Segoe UI Emoji, Noto Color Emoji, Apple Color Emoji);
  **Segoe UI Symbol / Cambria Math / DejaVu / Noto Sans Math** for mathematical and symbol text.
- **[Noto Sans CJK / Microsoft YaHei / Yu Gothic / Malgun Gothic](https://github.com/notofonts)** (via the
  OS) — Chinese, Japanese and Korean text.

**Audio, video and calls**
- **[FFmpeg](https://ffmpeg.org/)** (external binary) — video frame extraction, animated stickers, the
  AI's video frames, camera capture and H.264 encode/decode for calls.
- **[rodio](https://github.com/RustAudio/rodio)** and **[cpal](https://github.com/RustAudio/cpal)** —
  playback (voice notes, video tracks, call audio) and microphone capture.
- **[libopus](https://opus-codec.org/)** through `audiopus` and
  **[ogg](https://github.com/RustAudio/ogg)** — decoding voice notes and encoding call audio.
- The jitter buffer, echo cancellation and PCM plumbing are ours, written against the WebRTC literature.

**Rust crates**
- **[tokio](https://tokio.rs/)** — the async runtime;
  **[async-channel](https://github.com/smol-rs/async-channel)** for the session's event pipe.
- **[serde](https://serde.rs/) / [serde_json](https://serde.rs/)** — every wire and cache format.
- **[RustCrypto](https://github.com/RustCrypto)** — `aes-gcm` (the AI key at rest), `sha2`, `hmac`,
  `hkdf`, `getrandom`.
- **[ureq](https://github.com/algesten/ureq)** — HTTP for updates, profile photos and media.
- **[qrcode](https://github.com/kennytm/qrcode-rust)** — the pairing QR matrix.
- **[zip](https://github.com/zip-rs/zip2)** — sticker-pack archives.
- **[tracing](https://tokio.rs/tokio/tracing)** and **[anyhow](https://github.com/dtolnay/anyhow)** —
  logging and error plumbing.
- **[regex](https://github.com/rust-lang/regex)**, **[chrono](https://github.com/chronotope/chrono)**,
  **[base64](https://github.com/marshallpierce/rust-base64)** — text, dates, payloads.
- **[rfd](https://github.com/PolyMeilex/rfd)** — the native file picker.
- **[windows-sys](https://github.com/microsoft/windows-rs)** and
  **[tauri-winrt-notification](https://github.com/tauri-apps/winrt-notification)** — clipboard file drops,
  the Win32 tray (`Shell_NotifyIcon`) and toasts.
- **[windows](https://github.com/microsoft/windows-rs)** — the installer's shortcuts and uninstall entries
  through `IShellLink`/`IPersistFile` COM.
- **[winresource](https://github.com/nabijaczleweli/winresource)** — embeds `icon.ico` in the exe.
- **[notify-rust](https://github.com/hoodie/notify-rust)** — Linux desktop notifications.
- …and every other crate in `Cargo.lock`; `cargo tree` lists them all.

**Tools**
- **[Rust](https://www.rust-lang.org/)** and **[Cargo](https://doc.rust-lang.org/cargo/)**.
- **[OpenCode](https://opencode.ai)** — the agentic coding environment this was built in.
- **[DeepSeek V4.1 Flash](https://deepseek.com)** and **[GLM 5.3 Flash](https://z.ai)** — the models
  that wrote the code.
- **[GitHub Actions](https://github.com/features/actions)** and
  **[cargo-audit](https://github.com/rustsec/rustsec)** — the CI that builds, lints, tests and audits
  every release.

**And**
- **WhatsApp / Meta** — for the protocol this reimplements and the design language it matches. Not
  affiliated, not endorsed, and subject to their Terms of Service.

## License

AGPL-3.0-only. The distributed binary statically links `wacore-libsignal`, whose files carry Signal's
AGPL-3.0-only headers, so the combined work is AGPL; see [`LICENSE.md`](LICENSE.md) and
[`SECURITY.md`](SECURITY.md). Architecture notes and gotchas live in [`AGENTS.md`](AGENTS.md); history is
in [`CHANGELOG.md`](CHANGELOG.md) and the per-surface checklist in [`docs/PARITY.md`](docs/PARITY.md).
