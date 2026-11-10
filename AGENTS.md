# WA Desk — development notes

Feature history lives in [`CHANGELOG.md`](CHANGELOG.md); build steps and dependency/licence
information stay in `README.md`; the per-surface checklist is [`docs/PARITY.md`](docs/PARITY.md).

## What this is

A WhatsApp desktop client written in Rust: `wa-core` speaks the protocol, `wa-ui` decides every
string/label/plan, `apps/desktop-native` paints an `eframe`/`egui` window. No WebView, no JS.

| Crate | Role |
| --- | --- |
| `crates/wa-types` | serializable UI vocabulary (`MessageView`, `UiEvent`, …), serde only |
| `crates/wa-ui` | pure decisions: text/labels/emoji/icons, merge plans, HTML renderers + tests |
| `crates/wa-core` | protocol session (`whatsapp-rust`): messages, media, groups, status, calls |
| `apps/desktop-native` | the shipped window (`wa-desk-native`) |
| `vendor/whatsapp-rust`, `vendor/wacore-appstate`, `vendor/diesel`, `vendor/whatsapp-rust-sqlite-storage` | patched dependencies |

One-way dependency: `wa-core` knows nothing about the UI, `wa-ui` knows nothing about the session.

## Current state

**Version `0.2.3-rc.1`.** Releases are `<core>-rc.N`, N stops at 99 then the patch rolls
(`0.2.1-rc.99` -> `0.2.2-rc.1`); `updates::tests::prereleases_compare_in_order` pins the order.

`app.rs` is **~1,340 lines**; the rest is split into:

| Module | Holds |
| --- | --- |
| `fonts.rs` | `install_fonts`, `SYMBOL_FONTS`, emoji/CJK/Korean fallback lists |
| `wallpaper.rs` | the doodle sheet (`wallpaper_textile`, `paint_wallpaper`) |
| `buttons.rs` | `pill_button`, `pill_plus_button`, `rail_button`, `fab_button`, `action_button` |
| `icon_widgets.rs` | `Icon` enum + Material name table, `draw_icon`, `icon_button`, `forward_checkbox` |
| `paint.rs` | `palette_thumb`, `circular_avatar`, `sender_color`, `thumb_texture`, `prune_textures` |
| `screens/` | `calls`, `channels`, `communities`, `media`, `meta_ai`, `settings`, `status`, `group` |

**Live-compared against `web.whatsapp.com`** (each with a `target/live_*.jpg` beside ours): chat-list
filter row, Calls drawer, group info, Settings, empty conversation pane, chat rows, Status, Channels,
Communities, Media, You, Meta AI rail entry.

## Media path

- `video::prepare` pre-extracts frames with ffmpeg (10 fps, <=640 px, first 120 s) + `audio.wav`;
  `frames::Prefetch` decodes **12 frames ahead**, the viewer uploads 10 per paint, never re-uploads a
  cached frame, keeps a 48-entry LRU and protects the playhead window.
- `request_media` retries a `Loading` entry after **15 s** (a reply that never arrives used to leave a
  bubble on its inline thumbnail forever).
- A photo/video whose file is missing shows a **download disc** ("Tap to download …") which sends
  `FetchMedia`; a downloaded bubble opens the viewer on tap.
- `app::note` writes media failures and fetches slower than 2 s to `diagnostics.log` with no flag;
  `app::diag` (needs `WA_DESK_DIAG=1`) logs everything else. Cold-cache probe: 24 files in 18-647 ms.

## View-once, albums, ticks

- A `<unavailable type="view_once">` fanout paints a 74 px placeholder (circled `1`, "Photo",
  Tap to open / Opened / Open this on your phone / **View once** for our own bubble);
  `Model::open_viewer` consumes it in `view_once_open`.
- **Albums work** (`target/real_album_final.png`): `Model::album_span` uses `wa_ui::chat::album_at`'s
  rule (same side/sender, image|video, <=4 s between items, up to 9); `bubbles::album_bubble` paints the
  grid and must `scroll_to_rect` its own rect - its height differs from the bubbles it replaced.
- Ticks come from `UiEvent::Receipt` (`read`/`played`/`delivered`, translated in
  `wa-core/src/events.rs`); receipts only arrive for *new* reads, so a quiet run shows none.

## Where data lives

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\io.github.dirttw.wadesk\whatsapp.db` |
| Linux | `~/.local/share/io.github.dirttw.wadesk/whatsapp.db` |

Beside the session: `chats.json` (chat list + newest 80 bubbles per chat), `contacts.json`,
`media.json` (id -> path), `settings.json`, `ai.key`, `diagnostics.log`.
**App media cache:** `%APPDATA%\io.github.dirttw.wadesk-native\media` (not the session folder).
Deleting `whatsapp.db` unlinks locally only.

## Open items

1. **Captures needing the user** (WhatsApp Web ignores synthetic clicks): the media viewer, the sticker
   panel, a view-once message on live data, the call overlay.
2. **Protocol-limited, read-only by design**: linked devices / 2FA / block list, Channels'
   "Find channels to follow" rows, Meta AI (entry point + honest pane only).
3. **Sticker panel**: five-column grid, search pill, Create tile and footer match; the tab row's
   per-pack category shortcuts are not implemented.
4. **Unconfirmed by the reporter**: the viewer controls (arrows, scrub fallback, caption, speaker, speed,
   wheel zoom) and read-badge receipts.

## Verification habit

Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`
(132 native + 43 wa-core + 232 wa-ui) **and** take a capture before claiming a change. Prefer a
real-session capture (`WA_DESK_OPEN_CHAT="<chat>"`, plus `WA_DESK_SCROLL_TO=<index>`) over the demo
model whenever the change touches media or lists. Several fixes in this project shipped before they
were checked (a Settings panic, the camera badge, the media probe, the poster wiring, an ineffective
scroll hook) - a claim without a capture has been wrong here before.

## Gotchas that matter

- **egui is 0.37**: panels take the root `Ui` (`Panel::left(..).show(ui, ..)`; `SidePanel` is gone),
  `screen_rect` is `content_rect`, `Id::new` is `Id::unique`, and `--screenshot` uses
  `ctx.request_screenshot(callback)`.
- **Clock format** is globally `8:44 pm` (forced `en-US`, independent of the OS locale).
- **Bubbles are measured, not framed**: lay the body out, allocate exactly that size, then paint the
  rounded rect, sender, body and clock. A `Frame` lets a right-aligned stamp stretch the bubble.
- **Short chats sit at the bottom** (padding from the previous frame's measured height); the list also
  pins to the newest message every frame unless `suppress_bottom` is set for a jump.
- **Icons are Material Symbols rasterised with `resvg`** (`icons.rs`, cached per name/size/colour).
  Never draw icon glyphs from egui's fonts. The generated table has **no zoom glyph** - the viewer
  magnifiers are the search glyph plus a painted `+`/`-`.
- **Colour emoji** come from the system emoji face via `swash` (`emoji_glyph.rs`); a mask-only glyph
  falls back to text rather than a solid block.
- **Group gutters**: a 40 px sender column (photo or initials) at the head of each sender run; the tail
  corner follows the run (`Model::ends_run`), date chips come from `Model::starts_new_day`.
- **`~` means "not in the address book"** and only ever on group bubbles; the decision uses the number
  resolved for display, because a LID sender has no PN in its JID.
- **Live search** highlights with a `LayoutJob` on a yellow band (char-aligned lowercase matching).
- **The media viewer** is a full-window `Area`: measure the stage from `ctx.screen_rect()`, never from
  `available_height()` (it reports ~83 000 px inside an Area). Prev/next discs anchor to the screen, not
  the stage, or they slide between portrait and landscape images.
- **The AI loop** (`ai.rs`) is bring-your-own OpenAI-compatible key (default OpenCode Go), AES-256-GCM
  under `ai.key`; replies only to saved contacts, or to a group message that @mentions the account or
  quotes it, with human-like delays, one reaction per 3 min, sticker selection by a vision pass.
- **Calls**: `wa-core/src/calls.rs` owns VoIP; `mic.rs` (cpal) resamples to 16 kHz/960-sample frames,
  `camera.rs` uses ffmpeg + libx264, `callvideo.rs` decodes the peer with `ffmpeg -f h264 -i -` (two
  threads; ffmpeg buffers ~32 KB so video appears about a second in), `aec.rs` adds an NLMS filter over
  the playout reference.
- **Video playback is pre-decoded**: frames + `audio.wav`; the viewer indexes by wall clock and the audio
  sink is seeked, not re-decoded. Without ffmpeg it shows the poster and says so.
- **Updates** are pre-releases: the updater lists `/releases?per_page=20`, keeps the highest tag above
  `CARGO_PKG_VERSION`, downloads with `ureq` (64 kB chunks, progress bar on the button), renames the
  running exe to `.old`, spawns with `--restarted` and exits after 1.5 s.
- **The installer** (`apps/windows-installer`) is a stub binary with the app appended and a `WADSETUP`
  trailer; it copies to `%LOCALAPPDATA%\Programs\WA Desk` and writes shortcuts via `IShellLink`.
- **Notifications** are batched per chat (700 ms quiet, "3 messages - latest line"); toasts only for a
  message that arrives while the window is unfocused and the chat is not open. The tray is Windows-only.
- **Disk hygiene**: `[profile.dev] incremental = false`. The incremental cache reached **188 GB** here
  (a 113 GB project folder); build caches are disposable, `dist/` is not.

## Capturing screenshots without a session

`--preview --screenshot out.png` renders `Model::demo()`. Useful env hooks: `WA_DESK_SIZE=WxH`,
`WA_DESK_SCREENSHOT_DELAY`, `WA_DESK_SCREEN=group|calls|status|channels|communities|media|meta-ai`,
`WA_DESK_SETTINGS=1` + `WA_DESK_SETTINGS_SECTION=n`, `WA_DESK_VIEWER_IMAGE=<file>`, `WA_DESK_STICKERS=<dir>`,
`WA_DESK_STICKER_STORE=1`, `WA_DESK_EMOJI=1`, `WA_DESK_NO_CHAT=1`, `WA_DESK_MULTILINE=1`,
`WA_DESK_COMM_MANY=1`, `WA_DESK_UPDATE_DEMO=<percent>`, `WA_DESK_DIAG=1`.
Real session: `dist\WA Desk.exe --screenshot out.png` with `WA_DESK_OPEN_CHAT="<title>"` and
`WA_DESK_OPEN_VIDEO=1` / `WA_DESK_SCROLL_TO=<index>`.
