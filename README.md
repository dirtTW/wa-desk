# WA Desk

An unofficial WhatsApp desktop client written in Rust for **Windows** and **Linux**. It speaks
the WhatsApp Web multi-device protocol through
[`whatsapp-rust`](https://crates.io/crates/whatsapp-rust) and renders a Tauri 2 WebView UI
(WebView2 on Windows, WebKitGTK on Linux).

> **Disclaimer.** This is not affiliated with, endorsed by, or connected to WhatsApp or Meta.
> It is an independent reimplementation of the WhatsApp Web protocol. Using a custom client can
> violate Meta's Terms of Service and can get the linked account **banned**. Use a secondary
> account if you care about the primary one.

---

## Development note

**This project is 100% vibe-coded — the author does not write code.**

I'm not a programmer. I wanted a WhatsApp desktop client that actually looked and behaved like
the real thing, so I built it by instructing AI coding agents: describing what I wanted, looking
at the result, and saying "no — like *this*" until it worked. The models wrote every line of Rust
and JavaScript; my part was the direction, the taste, and the patience.

**Built with:**

- **DeepSeek V4.1 Flash** — primary coding model
- **GLM 5.3 Flash** — second coding model

What that means if you use it:

- It genuinely works, but it has **not had a security audit**. It handles end-to-end encrypted
  messages and stores credentials — read it before trusting it with anything sensitive, and use
  a secondary account.
- There will be bugs and rough edges. Issues and pull requests are welcome.
- If "an AI wrote this" isn't your thing, that's a fair opinion — but the source is open
  (AGPL-3.0-only), so you can read every line and judge it on what it does, not on how it was
  typed.

## What it does

A desktop client that mirrors WhatsApp Web's look and behaviour:

- QR and phone-number pairing, with a persistent SQLite session
- Chat list and history sync, plus media: photos, video, audio/voice notes, stickers, GIFs
- Send and receive text, media, replies, reactions, edits and deletes
- Groups (create, info, members), channels, and status / stories
- Full-screen media viewer, in-chat search, starred messages, chat export/import
- Optional AI auto-reply with your own OpenAI-compatible key
- In-app updates — Settings → Account checks for a newer release and installs it
  (signed, verified against a pinned key)
- System tray, desktop notifications, launch at login, light/dark theme

The full, always-current feature inventory, architecture notes, design gotchas and roadmap live
in [`AGENTS.md`](AGENTS.md); the change log is [`CHANGELOG.md`](CHANGELOG.md).

---

## Requirements

- Rust **1.94+** (edition 2024). Install with [rustup](https://rustup.rs/).
- **Windows**: MSVC build tools (Visual Studio "Desktop development with C++") and WebView2
  (preinstalled on Windows 10/11).
- **Linux** (Debian/Ubuntu):

  ```bash
  sudo apt install build-essential curl wget file \
      libwebkit2gtk-4.1-dev libgtk-3-dev libssl-dev \
      libxdo-dev librsvg2-dev libayatana-appindicator3-dev
  ```

  Fedora/Arch equivalents: `webkit2gtk4.1-devel`, `gtk3-devel`, `openssl-devel`,
  `libxdo-devel`, `librsvg2-devel`, `libayatana-appindicator-gtk3-devel`.

## Run from source

```bash
cargo run -p wa-desktop

# or with the Tauri CLI (hot reload for the frontend)
cargo install tauri-cli --version "^2"
cargo tauri dev
```

First launch shows a QR code. On your phone open
**WhatsApp → Settings → Linked devices → Link a device**, then scan the code, or use
*Link with phone number instead* and type the 8-character code.

If you unlink from inside the app, the local device store is removed and a fresh QR is
generated immediately. Check **Linked devices** on the phone and remove any stale `WA Desk`
entry before scanning again; the server-side unlink can fail while the phone is offline, but
the local cleanup still lets you pair a new session.

## Build installers

```bash
cargo tauri build
```

Outputs land in `target/release/bundle/`:

- Windows: `.msi` (WiX) and `.exe` (NSIS)
- Linux: `.deb`, `.rpm`, `.AppImage`

Cross-compiling is not supported by Tauri; build each target on its own OS.

The release profile favors wall time (`codegen-units = 16`, `lto = false`). For the smallest
binary, switch to `codegen-units = 1` and `lto = "thin"` at the cost of a slower build.

## Portable Windows build

Double-click `build-portable.bat` (or run it from a terminal). It builds the release binary
without requiring the Tauri CLI and creates:

- `dist\WA Desk.exe` — ready to run
- `dist\WA-Desk-portable-<version>.zip` — the same exe, plus a short readme and the licence

The exe is self-contained (UI assets are embedded). It still uses the system WebView2 runtime
included with Windows 10/11. Settings, chats and media stay in
`%APPDATA%\io.github.dirttw.wadesk`; copy that folder as well when moving to another PC. This is a
portable executable, not a fully portable data store.

For AI reading of GIFs/short videos, put `ffmpeg.exe` next to `WA Desk.exe` (or on `PATH`);
without it the client uses the video's embedded thumbnail.

---

## Updates

WA Desk checks for updates from inside the app: **Settings → Account →
Updates → Check for updates**. When a newer release exists the row becomes
*Update to vX*, which downloads the installer, verifies its minisign signature
against the public key pinned in `tauri.conf.json`, installs it and restarts.

The check reads `plugins.updater.endpoints`, in order:

1. `https://raw.githubusercontent.com/dirtTW/wa-desk/latest/latest.json` — a
   `latest` branch the release job keeps in step. GitHub's `/releases/latest`
   skips pre-releases, so this branch is what makes the RC channel work.
2. `https://github.com/dirtTW/wa-desk/releases/latest/download/latest.json` —
   the same manifest as an asset on the newest full release.

Both are written by the `release` job in
[`.github/workflows/build.yml`](.github/workflows/build.yml) on every `v*` tag.

---

## License

**GNU Affero General Public License v3.0 only (AGPL-3.0-only)** — see [`LICENSE.md`](LICENSE.md).
The AGPL is not a free choice: the binary statically links `wacore-libsignal`'s Signal-derived,
AGPL-3.0-only sources (see [Why AGPL?](#why-agpl)), so the whole combined work is AGPL.

If you convey a copy of the app (a release build, a prebuilt download), the AGPL obliges you to
make the complete corresponding source available under the same licence. `whatsapp-rust` is MIT
and carries its own disclaimer about protocol use; the AGPL grant covers the code, not the
WhatsApp service — using a custom client remains subject to Meta's Terms of Service.

### Third-party components

Direct Rust dependencies:

| Component | Version | License |
| --- | --- | --- |
| [`whatsapp-rust`](https://crates.io/crates/whatsapp-rust) (vendored) | 0.7.0 | MIT |
| `wacore` → `wacore-libsignal` (transitive, statically linked) | 0.7.0 | **AGPL-3.0-only** |
| [`tauri`](https://crates.io/crates/tauri) | 2.12.5 | Apache-2.0 OR MIT |
| [`tauri-plugin-autostart`](https://crates.io/crates/tauri-plugin-autostart) | 2.7.0 | Apache-2.0 OR MIT |
| [`tauri-plugin-notification`](https://crates.io/crates/tauri-plugin-notification) | 2.8.0 | Apache-2.0 OR MIT |
| [`tokio`](https://crates.io/crates/tokio) | 1.54.1 | MIT |
| [`serde`](https://crates.io/crates/serde) / [`serde_json`](https://crates.io/crates/serde_json) | 1.0.229 / 1.0.154 | MIT OR Apache-2.0 |
| [`anyhow`](https://crates.io/crates/anyhow) | 1.0.104 | MIT OR Apache-2.0 |
| [`async-channel`](https://crates.io/crates/async-channel) | 2.5.0 | Apache-2.0 OR MIT |
| [`base64`](https://crates.io/crates/base64) | 0.22.1 | MIT OR Apache-2.0 |
| [`qrcode`](https://crates.io/crates/qrcode) | 0.14.1 | MIT OR Apache-2.0 |
| [`chrono`](https://crates.io/crates/chrono) | 0.4.45 | MIT OR Apache-2.0 |
| [`flate2`](https://crates.io/crates/flate2) | 1.1.10 | MIT OR Apache-2.0 |
| [`ureq`](https://crates.io/crates/ureq) | 3.5.0 | MIT OR Apache-2.0 |
| [`aes-gcm`](https://crates.io/crates/aes-gcm) | 0.10.3 | Apache-2.0 OR MIT |
| [`webview2-com`](https://crates.io/crates/webview2-com) | 0.39.1 | MIT |
| [`windows`](https://crates.io/crates/windows) | 0.62.2 | MIT OR Apache-2.0 |
| [`tracing`](https://crates.io/crates/tracing) / [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber) | 0.1.44 / 0.3.23 | MIT |

Vendored source:

- `vendor/whatsapp-rust/` — MIT; the full text is in `vendor/whatsapp-rust/LICENSE`.
- `vendor/wacore-appstate/` — MIT (patched snapshot-MAC recovery).
- `wacore-libsignal` (from crates.io, statically linked) — **AGPL-3.0-only** for the 31
  files carrying Signal's header; see [Why AGPL?](#why-agpl).
- `apps/desktop/ui/vendor/material-web.js` — Material Web, Apache-2.0
  (<https://github.com/material-components/material-web>).
- `tests/e2e/node_modules/playwright-core/` — Apache-2.0; development/e2e only, not shipped.

Runtime components:

- **WebView2 runtime** (Windows) — Microsoft proprietary; preinstalled with Windows 10/11 and
  not bundled with the portable exe.
- **ffmpeg** (optional) — external, not bundled. The license depends on the ffmpeg build chosen
  by the user.

Full license texts are included with each crate in the Cargo registry; `Cargo.lock` records the
exact dependency set for a given build.

### Why AGPL?

`whatsapp-rust`'s `wacore` crate depends on **`wacore-libsignal`**, which vendors Signal's
libsignal sources. That crate declares `license = "MIT"` in its `Cargo.toml` — so `cargo`,
crates.io, `cargo-deny` and `cargo-about` all report it as MIT — but **31 of its files carry
Signal Messenger's `SPDX-License-Identifier: AGPL-3.0-only` header** and it ships no MIT
headers and no `LICENSE` file. The per-file headers, not the metadata, govern the licence
(upstream issue:
[`oxidezap/whatsapp-rust#1828`](https://github.com/oxidezap/whatsapp-rust/issues/1828)).

`wacore` depends on it *without* `optional = true`, so no feature flag can drop it, and Rust
links it into the executable:

```
wacore-libsignal ─▶ wacore ─▶ whatsapp-rust ─▶ wa-core ─▶ wa-desktop (WA Desk.exe)
```

That makes the distributed binary a combined work, and the AGPL requires the whole work to be
released under the AGPL with its source available. WA Desk therefore ships as **AGPL-3.0-only**.
If you only ever run the app yourself and never convey a copy, the distribution obligations do
not trigger — but the licence statement is honest about what the binary contains either way.

If the upstream metadata is ever corrected, or `wacore-libsignal` is replaced with a
permissively licensed equivalent, the project could be relicensed back.

---

## Credits

WA Desk is a thin layer on top of other people's work. Special thanks to everyone below — this
client would not exist without them.

### Protocol

- **[whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust)** by
  [jlucaso1](https://github.com/jlucaso1) — the WhatsApp Web protocol client this app is built on
  (`wacore`, `waproto`, `wacore-noise`, `wacore-appstate`, `whatsapp-rust-*`). This is the single
  biggest dependency and the reason the project is possible at all.
- **[Signal](https://signal.org/) / [libsignal](https://github.com/signalapp/libsignal)** — the
  Double Ratchet, X3DH and the crypto primitives behind end-to-end encryption
  (`wacore-libsignal`). Also the reason this app is AGPL.
- **[whatsmeow](https://github.com/tulir/whatsmeow)** and
  **[Baileys](https://github.com/WhiskeySockets/Baileys)** — the reference implementations that
  `whatsapp-rust` checks its protocol behaviour against.

### Desktop shell

- **[Tauri](https://tauri.app/)** — the desktop app framework, and the WebView it drives.
- **[WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)** (Microsoft) — the
  Windows renderer, preinstalled with Windows 10/11.
- **[WebKitGTK](https://webkitgtk.org/)** — the Linux renderer.

### Crates

- **[tokio](https://tokio.rs/)** — the async runtime.
- **[serde](https://serde.rs/)** / **[serde_json](https://serde.rs/)** — serialization.
- **[RustCrypto](https://github.com/RustCrypto)** — `aes-gcm`, `sha2`, `hmac`, `hkdf`.
- **[ureq](https://github.com/algesten/ureq)** — the blocking HTTP client.
- **[qrcode](https://github.com/kennytm/qrcode-rust)** — the pairing QR code.
- **[tracing](https://tokio.rs/tokio/tracing)** — logging.
- **[anyhow](https://github.com/dtolnay/anyhow)** — error handling.
- **[webview2-com](https://github.com/wravery/webview2-rs)** and
  **[windows](https://github.com/microsoft/windows-rs)** — the Win32 bindings.
- …and every other crate in `Cargo.lock`; `cargo tree` lists them all.

### Assets

- **[Material Symbols](https://fonts.google.com/icons)** (Google) — every icon in the UI.
- **[Material Web](https://github.com/material-components/material-web)** (Google) — the settings
  dialog and its controls.

### Tools

- **[OpenCode](https://opencode.ai)** — the agentic coding environment this was built in.
- **[DeepSeek V4.1 Flash](https://deepseek.com)** and **[GLM 5.3 Flash](https://z.ai)** — the
  models that wrote the code (see the [Development note](#development-note)).

### And

- **WhatsApp / Meta** — for the protocol this reimplements. Not affiliated, not endorsed, and
  subject to their Terms of Service.

