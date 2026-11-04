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
- **Media viewer**: filmstrip, arrow keys, Esc, and Save copies to `Downloads\WA Desk`; **videos
  play** in it (decoded with `ffmpeg`) with the official transport bar - play/pause, seek line,
  elapsed/total, mute and fullscreen.
- **Emoji and stickers**: full catalogue with search/recents, favourites and cached stickers, in
  the official picker (category tabs, rounded search, colour emoji rasterised from the system
  colour font).
- **Message actions**: reply, react, copy, forward, edit, delete (for me / everyone) and
  **star**; plus **chat actions**: mute, pin, mark unread, archive, clear, delete. Starred
  messages have their own screen, and the Chats menu can create a group or save a contact.
- **Status / stories** and **channels**, read-only for channels.
- **Send files**: paperclip picker, or Ctrl+V a file copied in Explorer; composer text becomes
  the caption.
- **Settings**: the official two-pane layout - profile header, General / Chats / AI / Updates /
  Help sections with icons and subtitles, Log out. The page is a set of cards with Material 3
  switches and radio rows; the chat list is replaced while it is open. Theme lives under Chats,
  General holds launch-at-login and the version; chat-history/group/status sync and relink sit
  next to it. Sync runs automatically on every connection (history batches, groups, statuses,
  chat state) and refreshes on a timer; profile photos are cached on disk.
- **Tray and notifications** (Windows): the WA Desk icon in the system tray with an Open / Quit
  menu, plus a desktop toast for messages that arrive while the window is in the background
  (a Settings switch, `--test-notification` from the CLI, session-bus notifications on Linux).
- **Official look**: WhatsApp's `--WDS-*` palette, Material Symbols rasterised with `resvg`,
  rounded composer and card menus, the green speech-bubble app icon in the window/taskbar/exe,
  and OS font fallbacks so Chinese (simplified and traditional), Japanese and Korean render.

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

Build each target on its own OS. The release profile favours wall time (`codegen-units = 16`,
`lto = false`).

## Where data lives

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\io.github.dirttw.wadesk-native\whatsapp.db` |
| Linux | `~/.local/share/io.github.dirttw.wadesk-native/whatsapp.db` |

`chats.json`, `settings.json` and downloaded media live beside it. Delete the database to unlink
locally (remove the device from the phone as well).

## Credits

WA Desk is a thin layer on other people's work; this client would not exist without them.

**Protocol**

- **[whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust)** by
  [jlucaso1](https://github.com/jlucaso1) — the WhatsApp Web client this app is built on
  (`wacore`, `waproto`, `wacore-noise`, `wacore-appstate`, `whatsapp-rust-*`).
- **[Signal](https://signal.org/) / [libsignal](https://github.com/signalapp/libsignal)** — the
  Double Ratchet, X3DH and the crypto primitives (`wacore-libsignal`); also why this app is AGPL.
- **[whatsmeow](https://github.com/tulir/whatsmeow)** and
  **[Baileys](https://github.com/WhiskeySockets/Baileys)** — the reference implementations
  `whatsapp-rust` checks its behaviour against.

**Window, icons and media**

- **[eframe](https://github.com/emilk/egui) / [egui](https://github.com/emilk/egui)** by
  [emilk](https://github.com/emilk) — the native window and everything painted in it.
- **[resvg](https://github.com/linebender/resvg)** — the pairing QR and the Material icons.
- **[image](https://github.com/image-rs/image)** — decoding photos, thumbnails and stickers.
- **[Material Symbols](https://fonts.google.com/icons)** (Google) — every icon, generated into
  `crates/wa-ui/src/icons_data.rs`.
- **Unicode emoji** and the OS emoji font (Segoe UI Emoji / Noto) — the picker and message glyphs.

**Crates**

- **[tokio](https://tokio.rs/)** — async runtime.
- **[serde](https://serde.rs/) / [serde_json](https://serde.rs/)** — serialization.
- **[RustCrypto](https://github.com/RustCrypto)** — `aes-gcm`, `sha2`, `hmac`, `hkdf`.
- **[ureq](https://github.com/algesten/ureq)** — HTTP for updates and profile photos.
- **[qrcode](https://github.com/kennytm/qrcode-rust)** — the pairing QR matrix.
- **[tracing](https://tokio.rs/tokio/tracing)** — logging.
- **[anyhow](https://github.com/dtolnay/anyhow)** — error handling.
- **[regex](https://github.com/rust-lang/regex)**,
  **[chrono](https://github.com/chronotope/chrono)**,
  **[base64](https://github.com/marshallpierce/rust-base64)** and
  **[async-channel](https://github.com/smol-rs/async-channel)** — text, dates, payloads, channels.
- **[rfd](https://github.com/PolyMeilex/rfd)** — the native file picker.
- **[windows-sys](https://github.com/microsoft/windows-rs)** — clipboard files (Ctrl+V) and the
  installer's shortcuts (COM). See also Microsoft's
  [`windows-rs`](https://github.com/microsoft/windows-rs) project.
- …and every other crate in `Cargo.lock`; `cargo tree` lists them all.

**Tools**

- **[OpenCode](https://opencode.ai)** — the agentic coding environment this was built in.
- **[DeepSeek V4.1 Flash](https://deepseek.com)** and **[GLM 5.3 Flash](https://z.ai)** — the
  models that wrote the code (see the development note above).

**And**

- **WhatsApp / Meta** — for the protocol this reimplements. Not affiliated, not endorsed, and
  subject to their Terms of Service.

## License

AGPL-3.0-only. The distributed binary statically links `wacore-libsignal`, whose files carry
Signal's AGPL-3.0-only headers, so the combined work is AGPL; see [`LICENSE.md`](LICENSE.md) and
[`SECURITY.md`](SECURITY.md). Architecture notes, gotchas and the roadmap live in
[`AGENTS.md`](AGENTS.md); the change log is [`CHANGELOG.md`](CHANGELOG.md).
