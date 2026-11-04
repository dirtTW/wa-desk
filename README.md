# WA Desk

An unofficial WhatsApp desktop client in **Rust** for Windows and Linux. It speaks the WhatsApp
Web multi-device protocol through [`whatsapp-rust`](https://crates.io/crates/whatsapp-rust) and
draws its window with `eframe`/`egui` — **no WebView, no HTML, no JavaScript anywhere in the
repository**.

> **Disclaimer.** Not affiliated with, endorsed by, or connected to WhatsApp or Meta. Using a
> custom client can violate Meta's Terms of Service and can get the linked account **banned**.
> Use a secondary account if you care about the primary one.

---

## Development note

**This project is 100% vibe-coded — the author does not write code.** I described what I wanted,
looked at the result and said "no — like *this*" until it worked.

**Built with:** DeepSeek V4.1 Flash (primary) and GLM 5.3 Flash.

It works, but it has **not had a security audit**. It handles end-to-end encrypted messages and
stores credentials: read it before trusting it with anything sensitive, and use a secondary
account. Issues and pull requests are welcome.

## What it does

- **Linking**: QR or phone-number pairing code, persistent SQLite session, live connection state.
- **Chat list like the phone**: 58 px rail with unread/unseen badges and your profile photo,
  kebab menu, green new-chat FAB, rounded search, and **All / Groups / Communities / Unread**
  filters with live counts. Rows show avatars (real photos when available), previews, clocks and
  unread pills, sorted by last message. The list is cached to `chats.json` and backfilled in
  batches, so a restart keeps it.
- **Groups and communities**: every group the account participates in, by name, with history and
  the Communities filter. **Read badges sync both ways** with the phone.
- **Conversation**: text, typing indicator, tails, sender colours, inline clocks; in-chat search
  with match counter and highlight; **documents as the official card**; photos and stickers.
- **Voice notes and audio**: official-style player bubble (play/pause, waveform, elapsed/total)
  with OGG/Opus decoded through libopus, so phone recordings play in the window
- **Media viewer, rebuilt against the official screen**: sender avatar, name and "Today at 7:16 am"
  in the header, the reply/star/forward/download/menu/close toolbar, prev/next arrows in the
  gutters, the transport bar **inside** the frame (play/pause, draggable seek line, `1.0x`, mute,
  fullscreen), a `1 of 2` counter and a filmstrip with the open item ringed green. Arrows page,
  Esc closes, Space plays; Save copies to `Downloads\WA Desk`.
- **Video that plays smoothly**: the clip's own frame rate is read with `ffprobe` (12-30 fps), the
  frames are extracted at that rate and decoded **ahead of the playhead on a worker thread**, so
  the paint loop only uploads ready frames and never blocks on the disk (the class of bug behind
  egui#7860). `ffmpeg` runs with `CREATE_NO_WINDOW`, so no console window flashes.
- **Emoji and stickers**: full catalogue with search/recents, favourites and cached stickers, in
  the official picker (40 px cells, 30 px glyphs, category tabs, rounded search). Every emoji is a
  colour glyph rasterised from the system colour font (`swash`) into a padded square cell, so text
  and emoji line up and neighbours never touch. **Animated WebP stickers animate** (their RIFF
  `ANIM` chunk is detected and the frames are looped like a GIF), and GIFs animate inline.
- **Message actions**: reply, react, copy, forward, edit, delete (for me / everyone) and
  **star**; plus **chat actions**: mute, pin, mark unread, archive, clear, delete. Starred
  messages have their own screen, and the Chats menu can create a group or save a contact.
- **Status / stories** and **channels**, read-only for channels.
- **Send files**: paperclip picker, or Ctrl+V a file copied in Explorer; composer text becomes
  the caption.
- **AI assistant**: bring your own OpenAI-compatible key (encrypted with AES-256-GCM), on/off and
  `/ai` requirement toggles, the casual Indonesian "gw/lu" persona, replies to DMs and to groups
  that @mention you, a typing indicator, human-like delays and a per-chat reply/reaction cooldown.
- **Settings**: the official two-pane layout - profile header, General / Chats / AI / Updates /
  Help sections with icons and subtitles, Log out. The page is a set of cards with Material 3
  switches and radio rows; the chat list is replaced while it is open. Theme lives under Chats,
  General holds launch-at-login and the version; chat-history/group/status sync and relink sit
  next to it. Sync runs automatically on every connection (history batches, groups, statuses,
  chat state) and refreshes on a timer; profile photos are cached on disk.
- **Sticker picker**: the official panel with Recents / Favourites / Packs tabs, store search, the
  Create tile and the shared Emoji/GIF/Stickers footer; the GIF tab lists the chat and re-sends
  one, and Create turns a picture into a 512×512 WebP sticker and sends it.
- **Voice and video calls**: start from the chat header or the kebab, accept/decline a call, mute
  and hang up; audio is captured at 16 kHz and plays through a dedicated sink, the peer's H.264
  video is decoded into the stage, and your own camera is captured, encoded and sent with a
  mirrored self-view thumbnail.
- **Tray and notifications** (Windows): the WA Desk icon in the system tray with an Open / Quit
  menu, written on `Shell_NotifyIcon` directly (no GTK in the dependency graph), plus a desktop
  toast for messages that arrive while the window is in the background (a Settings switch,
  `--test-notification` from the CLI, session-bus notifications on Linux).
- **Official look**: WhatsApp's palette sampled live from `web.whatsapp.com` (`#0B141A` wallpaper,
  `#111B21` panels, `#202C33` headers and incoming bubbles, `#005C4B` outgoing), the faint doodle
  wallpaper tiled behind the messages, Material Symbols rasterised with `resvg`, the rounded
  composer with emoji/stickers inside the field, reactions straddling the bubble's edge, and OS
  font fallbacks (symbols, Chinese simplified + traditional, Japanese, Korean) so a nickname like
  `𝕋𝕒𝕦𝕗𝕚𝕢` renders instead of six boxes.
- **Unsaved participants**: a group bubble's sender row reads `~ Name` with their number beside it;
  the chat list and header keep plain names.
- **Bounded memory**: media and sticker textures decode at the size they paint at and are pruned
  by least-recently-painted use, video frames are capped and released when the viewer closes, and
  `WA_DESK_DIAG=1` logs a `memory:` line every ten seconds so a session can be watched.

**History caveat.** WhatsApp only pushes older message history to a newly linked device. The app
asks for the recent chats in batches and on open; Settings → Chat history can unlink so a fresh
pair imports everything.

## How it is put together

```
crates/wa-types/      serializable vocabulary shared by protocol and UI
crates/wa-core/       the session: protocol, Signal, media, history, groups, status
crates/wa-ui/         every UI decision: strings, labels, plans, timings (+ tests)
apps/desktop-native/  the window: eframe/egui, driven by UiEvent and Command
apps/windows-installer/  the setup stub that installs the app
```

`wa-core` knows nothing about the UI and `wa-ui` knows nothing about the session, so either can
be reused alone. The UI is verified without a display:

```bash
cargo run -p wa-desktop-native -- --selftest
# {"channels":1,"chats":3,"delete":"Delete message?","delete_everyone":true,"emoji":160,
#  "first":"Team","forward":1,"group_members":2,"linked":true,"photo_body":"Photo",
#  "preview":"ok","reactions":2,"reply":false,"search":"1/1","stamp":"9:40 am",
#  "status":"Connected","status_groups":2,"stickers":0,"theme":"light","viewer_items":1}
```

## Install on Windows

Download **`WA-Desk-Setup.exe`** from the [releases](https://github.com/dirtTW/wa-desk/releases)
and run it — no admin, no MSI/WiX. It installs to `%LOCALAPPDATA%\Programs\WA Desk`, adds Start
Menu and Desktop shortcuts, starts the app, and leaves `Uninstall WA Desk.cmd` behind. Linux
ships the plain `wa-desk-native` binary.

## Requirements

- Rust **1.94+** (edition 2024) — install with [rustup](https://rustup.rs/).
- **Windows**: MSVC build tools ("Desktop development with C++").
- **Linux** (Debian/Ubuntu): `sudo apt install build-essential libssl-dev pkg-config
  libasound2-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev` (ALSA is needed for audio
  playback). Fedora/Arch: `openssl-devel`, `alsa-lib-devel`, `libxkbcommon-devel`,
  `wayland-devel`, `mesa-libGL-devel`.
- **`ffmpeg`** (optional, on `PATH`, next to the exe, or `WA_DESK_FFMPEG`) - video playback and the
  AI's video frames use it; without it clips keep the poster.

## Run from source

```bash
cargo run -p wa-desktop-native
cargo run -p wa-desktop-native -- --selftest                     # headless UI check
cargo run -p wa-desktop-native -- --preview --screenshot ui.png  # sample chat, no account
```

First launch shows a QR code: on the phone open **WhatsApp → Settings → Linked devices → Link a
device**, or use *Link with phone number instead*.

## Build

```bash
cargo build --release -p wa-desktop-native
# Windows: target/release/wa-desk-native.exe
# Windows installer: cargo run -p wa-windows-installer --bin pack-setup -- \
#   target/release/wa-desk-setup.exe target/release/wa-desk-native.exe "WA-Desk-Setup.exe"
```

Build each target on its own OS. The release profile favours **size**: fat LTO, one codegen unit,
stripped symbols and `panic = "abort"` take the exe to ~25 MB. CI builds with `--profile ci`, which
skips the optimisation so lint and test runs stay fast.

## Where data lives

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\io.github.dirttw.wadesk-native\whatsapp.db` |
| Linux | `~/.local/share/io.github.dirttw.wadesk-native/whatsapp.db` |

`chats.json`, `settings.json` and downloaded media live beside it. Delete the database to unlink
locally (remove the device from the phone as well).

## Versioning

Releases are pre-releases: `0.2.1-rc.4` is the fourth release candidate of `0.2.1`. Every entry in
[`CHANGELOG.md`](CHANGELOG.md) names the mechanism and how it was verified (tests, screenshots,
`--selftest` output). The in-app updater compares the numeric core first, then the `rc.N` number,
so `0.2.2-rc.1` is offered over `0.2.1-rc.9`.

## Credits

WA Desk is a thin layer on other people's work. Every project below is used by the shipped client;
thank you to all of them.

**Protocol and crypto**

- **[whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust)** by
  [jlucaso1](https://github.com/jlucaso1) — the WhatsApp Web multi-device client this app is built
  on: `whatsapp-rust` (patched in `vendor/whatsapp-rust`), `wacore`, `waproto`, `wacore-noise`,
  `wacore-appstate` (patched in `vendor/wacore-appstate`), `wacore-binary`, `wacore-libsignal` and
  the `whatsapp-rust-*` helpers.
- **[Signal](https://signal.org/) / [libsignal](https://github.com/signalapp/libsignal)** — the
  Double Ratchet, X3DH and the primitives behind the end-to-end encryption; also why this app is
  AGPL.
- **[whatsmeow](https://github.com/tulir/whatsmeow)** and
  **[Baileys](https://github.com/WhiskeySockets/Baileys)** — the reference implementations
  `whatsapp-rust` checks itself against.
- **[SQLite](https://sqlite.org/)** through `rusqlite` — where the linked session lives.

**Window, text and pictures**

- **[eframe](https://github.com/emilk/egui) / [egui](https://github.com/emilk/egui)** by
  [emilk](https://github.com/emilk) — the native window and everything painted in it, plus
  `epaint` and `ecolor`.
- **[resvg](https://github.com/linebender/resvg) / [tiny-skia](https://github.com/linebender/tiny-skia)**
  — rasterising the pairing QR code and the Material icons.
- **[swash](https://github.com/dfrg/swash)** — turns colour-emoji glyphs (COLR/CPAL, CBDT) into
  RGBA textures.
- **[image](https://github.com/image-rs/image)** — decoding photos, posters, GIFs and stickers.
- **[Material Symbols](https://fonts.google.com/icons)** by Google — every icon, generated into
  `crates/wa-ui/src/icons_data.rs` (`cargo run -p wa-ui --example gen_icons`).
- **Unicode emoji** and the OS colour emoji face (Segoe UI Emoji, Noto Color Emoji, Apple Color
  Emoji) — the picker and message glyphs; **Segoe UI Symbol / Cambria Math / DejaVu / Noto Sans
  Math** for mathematical and symbol text.
- **[Noto Sans CJK / Microsoft YaHei / Yu Gothic / Malgun Gothic](https://github.com/notofonts)**
  (via the OS) — Chinese, Japanese and Korean text.

**Audio, video and calls**

- **[FFmpeg](https://ffmpeg.org/)** (external binary) — video frame extraction, animated stickers,
  the AI's video frames, camera capture and H.264 encode/decode for calls.
- **[rodio](https://github.com/RustAudio/rodio)** and **[cpal](https://github.com/RustAudio/cpal)**
  — audio playback (voice notes, video tracks, call audio) and microphone capture.
- **[libopus](https://opus-codec.org/)** through `audiopus` and **[ogg](https://github.com/RustAudio/ogg)**
  — decoding voice notes and encoding call audio.
- **[Web Audio / WebCodecs](https://www.w3.org/TR/webcodecs/)**-style PCM plumbing is ours, but the
  jitter buffer and AEC were written against the WebRTC literature.

**Rust crates**

- **[tokio](https://tokio.rs/)** — the async runtime; **[async-channel](https://github.com/smol-rs/async-channel)**
  for the session's event pipe.
- **[serde](https://serde.rs/) / [serde_json](https://serde.rs/)** — every wire and cache format.
- **[RustCrypto](https://github.com/RustCrypto)** — `aes-gcm` (the AI key at rest), `sha2`, `hmac`,
  `hkdf`, `getrandom`.
- **[ureq](https://github.com/algesten/ureq)** — HTTP for updates, profile photos and media.
- **[qrcode](https://github.com/kennytm/qrcode-rust)** — the pairing QR matrix.
- **[zip](https://github.com/zip-rs/zip2)** — sticker-pack archives.
- **[tracing](https://tokio.rs/tokio/tracing)** and **[anyhow](https://github.com/dtolnay/anyhow)**
  — logging and error plumbing.
- **[regex](https://github.com/rust-lang/regex)**, **[chrono](https://github.com/chronotope/chrono)**,
  **[base64](https://github.com/marshallpierce/rust-base64)** — text, dates, payloads.
- **[rfd](https://github.com/PolyMeilex/rfd)** — the native file picker.
- **[windows-sys](https://github.com/microsoft/windows-rs)** and
  **[tauri-winrt-notification](https://github.com/tauri-apps/winrt-notification)** — clipboard file
  drops, the Win32 tray (`Shell_NotifyIcon`), toasts, and the installer's shortcuts.
- **[notify-rust](https://github.com/hoodie/notify-rust)** — Linux desktop notifications.
- **[windows](https://github.com/microsoft/windows-rs)** (the `windows` crate) — the installer's
  shortcut and uninstall entries through `IShellLink`/`IPersistFile` COM.
- **[winresource](https://github.com/nabijaczleweli/winresource)** — embeds `icon.ico` in the exe.
- …and every other crate in `Cargo.lock`; `cargo tree` lists them all.

**Tools**

- **[Rust](https://www.rust-lang.org/)** and **[Cargo](https://doc.rust-lang.org/cargo/)**.
- **[OpenCode](https://opencode.ai)** — the agentic coding environment this was built in.
- **[DeepSeek V4.1 Flash](https://deepseek.com)** and **[GLM 5.3 Flash](https://z.ai)** — the
  models that wrote the code (see the development note above).
- **[GitHub Actions](https://github.com/features/actions)** and **[cargo-audit](https://github.com/rustsec/rustsec)**
  — the CI that builds, lints, tests and audits every release.

**And**

- **WhatsApp / Meta** — for the protocol this reimplements and the design language it matches. Not
  affiliated, not endorsed, and subject to their Terms of Service.

## License

AGPL-3.0-only. The distributed binary statically links `wacore-libsignal`, whose files carry
Signal's AGPL-3.0-only headers, so the combined work is AGPL; see [`LICENSE.md`](LICENSE.md) and
[`SECURITY.md`](SECURITY.md). Architecture notes, gotchas and the roadmap live in
[`AGENTS.md`](AGENTS.md); the change log is [`CHANGELOG.md`](CHANGELOG.md).
