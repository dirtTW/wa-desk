# WA Desk

An unofficial WhatsApp desktop client written in **Rust**, for **Windows** and **Linux**. It
speaks the WhatsApp Web multi-device protocol through
[`whatsapp-rust`](https://crates.io/crates/whatsapp-rust) and draws its window with
`eframe`/`egui` — **no WebView, no HTML, no JavaScript anywhere in the repository**.

> **Disclaimer.** This is not affiliated with, endorsed by, or connected to WhatsApp or Meta.
> It is an independent reimplementation of the WhatsApp Web protocol. Using a custom client can
> violate Meta's Terms of Service and can get the linked account **banned**. Use a secondary
> account if you care about the primary one.

---

## Development note

**This project is 100% vibe-coded — the author does not write code.**

I'm not a programmer. I wanted a WhatsApp desktop client that behaved the way the real thing
does, so I built it by instructing AI coding agents: describing what I wanted, looking at the
result, and saying "no — like *this*" until it worked. The models wrote every line of Rust; my
part was the direction, the taste, and the patience.

**Built with:**

- **DeepSeek V4.1 Flash** — primary coding model
- **GLM 5.3 Flash** — second coding model

What that means if you use it:

- It genuinely works, but it has **not had a security audit**. It handles end-to-end encrypted
  messages and stores credentials — read it before trusting it with anything sensitive, and use
  a secondary account.
- There will be bugs and rough edges. Issues and pull requests are welcome.
- If "an AI wrote this" isn't your thing, that's a fair opinion — but the source is open
  (AGPL-3.0-only), so you can read every line and judge it on what it does.

## What it does today

The native window (`wa-desk-native`) covers:

- **Linking**: QR scan or phone-number pairing code, with a persistent SQLite session, and the
  connection state always visible
- **Chats**: the list from history sync, search, previews, clock stamps, unread badges that
  clear when another device reads the chat
- **Conversation**: text, send and receive, typing indicator, own/other bubbles, sender + time
  headers, reading a history replay
- **In-chat search**: the header's search bar with a match counter, previous/next stepping,
  yellow highlighting inside the bubble and on the matched bubble, Esc to close
- **Media**: photos decoded into the window (one download per message), stickers painted
  without a bubble, placeholders for video / audio / document
- **Media viewer**: full-window stage, filmstrip, prev/next and arrow keys, Esc to close, and
  Save copies the file to `Downloads\WA Desk`
- **Emoji picker**: the full catalogue with search and recents, inserting into the composer
- **Sticker picker**: the account's favourites from app state plus the stickers already
  cached from chats; clicking one sends it
- **Status / stories**: poster rail with unseen/seen rings, story viewer with per-frame progress,
  background colours, prev/next and auto-advance
- **Channels**: followed newsletters with their follower counts and read-only posts
- **Group info**: subject, description, member count and the member list with admin chips
- **Message actions**: right-click a bubble for reply, react, copy, forward, edit (your own
  text) and delete; a reply/edit card sits above the composer, deletes ask "for me" or "for
  everyone", forwarding offers a multi-select chat picker, and reactions show as pills under
  the bubble (ours in blue)
- **Auto-update**: checks GitHub releases on launch (pre-releases included), shows the version
  in Settings → Updates, and downloads + swaps the platform binary on restart
- **Settings**: theme (Light / Dark / System) and the AI form, validated by the same rules the
  rest of the app uses
- **The official look**: WhatsApp's `--WDS-*` palette, avatars, unread pills, bubbles with their
  tail and inline clock, wallpaper, rounded composer and the message menu - all drawn with
  `egui`/`Painter`, no images and no JavaScript

**What is no longer in the tree** (removed in `v0.2.0-rc.57`): the Tauri shell, its
WebView (`wry`, `webview2-com`, `webkit2gtk`), `tauri-plugin-{notification,autostart,updater}`,
the MSI/NSIS bundling and the 13-file JavaScript front end - several thousand lines of JS
and the whole HTML/CSS tree. `Cargo.lock` now has zero `tauri`, `wry`, `webview2` and
`webkit2gtk` entries; the only new dependencies are `eframe`, `egui`, `resvg` (QR) and
`image` (photo decoding). In `v0.2.0-rc.60` the last dev tooling went too: all 23
`.mjs`/`.ps1` scripts and `package.json` were deleted, and the surviving icon generator is
the Rust example `cargo run -p wa-ui --example gen_icons`. **The repository now has zero
`.js`, `.mjs`, `.ps1` and `.bat` files** - Rust is the only language left.
Planned next, in rough order: notifications + tray + autostart, the AI auto-reply loop (it
used to live in the removed WebView shell), and calls on a native audio/video path.

## How it is put together

```
crates/wa-types/   serializable vocabulary shared by the protocol and the UI
crates/wa-core/    the session: protocol, Signal, media, history, groups, status
crates/wa-ui/      every decision the UI makes: strings, labels, plans, timings (+ tests)
apps/desktop-native/  the window: eframe/egui, driven by UiEvent and Command
```

`wa-core` knows nothing about the UI and `wa-ui` knows nothing about the session, so a second
front end (or a daemon) can reuse either. The native window is a thin painter: it receives
`UiEvent`s from the session and asks `wa-ui` what to write.

The UI is verified without a display:

```bash
cargo run -p wa-desktop-native -- --selftest
# {"channels":1,"chats":3,"delete":"Delete message?","delete_everyone":true,"emoji":160,
#  "first":"Team","forward":1,"group_members":2,"linked":true,"photo_body":"Photo",
#  "preview":"ok","reactions":2,"reply":false,"search":"1/1","stamp":"9:40 am",
#  "status":"Connected","status_groups":2,"stickers":0,"theme":"light","viewer_items":1}
```

`--selftest` folds a scripted event sequence through the same model the window uses — pairing
gate, chats, unread, media lifecycle, status viewer, channels, settings and group info — and
exits non-zero if anything disagrees.

## Install on Windows

Download **`WA-Desk-Setup.exe`** from the [releases page](https://github.com/dirtTW/wa-desk/releases)
and run it. It needs no admin rights and no MSI/WiX runtime:

- installs per-user into `%LOCALAPPDATA%\Programs\WA Desk`
- adds Start Menu and Desktop shortcuts
- starts the app
- leaves `Uninstall WA Desk.cmd` beside it (and Settings → Updates keeps it current)

Linux ships the plain `wa-desk-native` binary.

## Requirements

- Rust **1.94+** (edition 2024). Install with [rustup](https://rustup.rs/).
- **Windows**: MSVC build tools (Visual Studio "Desktop development with C++").
- **Linux** (Debian/Ubuntu):

  ```bash
  sudo apt install build-essential libssl-dev pkg-config \
      libxkbcommon-dev libwayland-dev libgl1-mesa-dev
  ```

  Fedora/Arch equivalents: `openssl-devel`, `libxkbcommon-devel`, `wayland-devel`,
  `mesa-libGL-devel`.

## Run from source

```bash
cargo run -p wa-desktop-native
cargo run -p wa-desktop-native -- --selftest    # headless UI check
# render a sample chat with no linked account and save a PNG
cargo run -p wa-desktop-native -- --preview --screenshot preview.png
```

First launch shows a QR code. On your phone open
**WhatsApp → Settings → Linked devices → Link a device**, then scan, or use *Link with phone
number instead* and type the 8-character code.

## Build

```bash
cargo build --release -p wa-desktop-native
# Windows output: target/release/wa-desk-native.exe
```

Build each target on its own OS. The release profile favours wall time (`codegen-units = 16`,
`lto = false`); switch to `codegen-units = 1` and `lto = "thin"` for the smallest binary at the
cost of a slower build.

## Where data lives

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\io.github.dirttw.wadesk-native\whatsapp.db` |
| Linux | `~/.local/share/io.github.dirttw.wadesk-native/whatsapp.db` |

Delete that file to unlink locally (any other linked device still shows this one until you
remove it from the phone). Settings and the downloaded media live beside it.

## Credits

WA Desk is a thin layer on top of other people's work. Special thanks to everyone below - this
client would not exist without them.

### Protocol

- **[whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust)** by
  [jlucaso1](https://github.com/jlucaso1) - the WhatsApp Web protocol client this app is built
  on (`wacore`, `waproto`, `wacore-noise`, `wacore-appstate`, `whatsapp-rust-*`). This is the
  single biggest dependency and the reason the project is possible at all.
- **[Signal](https://signal.org/) / [libsignal](https://github.com/signalapp/libsignal)** - the
  Double Ratchet, X3DH and the crypto primitives behind end-to-end encryption
  (`wacore-libsignal`). Also the reason this app is AGPL.
- **[whatsmeow](https://github.com/tulir/whatsmeow)** and
  **[Baileys](https://github.com/WhiskeySockets/Baileys)** - the reference implementations that
  `whatsapp-rust` checks its protocol behaviour against.

### Window and assets

- **[eframe](https://github.com/emilk/egui)** / **[egui](https://github.com/emilk/egui)** by
  [emilern](https://github.com/emilk) - the native window and everything painted in it. No
  WebView, no HTML, no JavaScript.
- **[resvg](https://github.com/linebender/resvg)** - rasterising the pairing QR into the window.
- **[image](https://github.com/image-rs/image)** - decoding photos, posters and stickers.
- **[Material Symbols](https://fonts.google.com/icons)** (Google) - every icon in the UI
  (generated into `crates/wa-ui/src/icons_data.rs`).
- **[Unicode emoji](https://unicode.org/emoji/)** - the picker catalogue from the Unicode
  character database, and the fonts that draw each glyph.

### Crates

- **[tokio](https://tokio.rs/)** - the async runtime.
- **[serde](https://serde.rs/)** / **[serde_json](https://serde.rs/)** - serialization.
- **[RustCrypto](https://github.com/RustCrypto)** - `aes-gcm`, `sha2`, `hmac`, `hkdf`.
- **[ureq](https://github.com/algesten/ureq)** - the blocking HTTP client.
- **[qrcode](https://github.com/kennytm/qrcode-rust)** - the pairing QR code matrix.
- **[tracing](https://tokio.rs/tokio/tracing)** - logging.
- **[anyhow](https://github.com/dtolnay/anyhow)** - error handling.
- **[regex](https://github.com/rust-lang/regex)**, **[chrono](https://github.com/chronotope/chrono)**,
  **[base64](https://github.com/marshallpierce/rust-base64)** and
  **[async-channel](https://github.com/smol-rs/async-channel)** - text, dates, payloads and
  channels.
- ...and every other crate in `Cargo.lock`; `cargo tree` lists them all.

### Tools

- **[OpenCode](https://opencode.ai)** - the agentic coding environment this was built in.
- **[DeepSeek V4.1 Flash](https://deepseek.com)** and **[GLM 5.3 Flash](https://z.ai)** - the
  models that wrote the code (see the [Development note](#development-note)).

### And

- **WhatsApp / Meta** - for the protocol this reimplements. Not affiliated, not endorsed, and
  subject to their Terms of Service.

## License

AGPL-3.0-only. The distributed binary statically links `wacore-libsignal`, whose files carry
Signal's AGPL-3.0-only headers, so the combined work is AGPL; see [`LICENSE.md`](LICENSE.md) and
[`SECURITY.md`](SECURITY.md). Third-party crates and their licenses are listed in the credits
above and in `Cargo.lock`.

The full feature inventory, architecture notes, design gotchas and roadmap live in
[`AGENTS.md`](AGENTS.md); the change log is [`CHANGELOG.md`](CHANGELOG.md).
