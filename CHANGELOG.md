# Changelog

Newest first. Each entry keeps the mechanism (files, protocol constants, test
names) and the verification signal. Durable design notes, the feature inventory
and the roadmap live in [`AGENTS.md`](AGENTS.md).

## Recent releases

The last eight releases are kept in full. Older ones are condensed to one line each;
`git log` and the annotated tags carry the full text.

## [0.2.3-rc.2] - 2026-11-14

- **Dependency refresh: the four Dependabot PRs, rebased onto `0.2.3-rc.1` and landed as one batch.**
  - `getrandom` 0.3 -> 0.4 (`apps/desktop-native/Cargo.toml`). The only call site is `ai.rs`'s
    `getrandom::fill(&mut bytes)`, which kept its shape in 0.4. `0.3.4` still resolves in
    `Cargo.lock` through `rand_core 0.9` -> `rand` -> `webrtc-*` (whatsapp-rust), so both versions
    ship and nothing had to be pinned back.
  - `resvg` 0.45 -> 0.49 (pulling `usvg` 0.49). `icons.rs`, `gate.rs` and `examples/gen_icon.rs` keep
    the same `Tree::from_str(&svg, &Options::default())` then `resvg::render` path, and the icon cache
    key (name, size, colour) is unchanged, so no glyph raster is re-keyed.
  - `flate2` 1.1.11, `ureq` 3.5.1 (+`ureq-proto` 0.7.1) and `zip` 9.4.1: `Cargo.lock` only, behind the
    updater (`ureq`) and the installer pack path (`zip`).
  - CI: `actions/cache` 4 -> 6 in all three jobs of `.github/workflows/build.yml`.
- **`clippy -D warnings` on the 1.100 toolchain** (stable moved to 1.100.0 on 2026-11-12) flagged
  `needless_range_loop` in `conversation.rs`'s `WA_DESK_DIAG` media note, which the 1.99 lint set had
  accepted. The loop is now `row.messages.iter().enumerate().skip(index).take(3)` - the same three
  neighbours, with no `#[allow]`.
- Verified by `build` run 43544532034 on `aa1f960`, whose tree is byte-identical to `main`'s `141de37`:
  `cargo fmt --check`, `clippy -D warnings` on 1.100, `cargo audit`, `cargo test --workspace` plus the
  headless `--selftest`, and both release builds with the Windows installer. The run before it, on the
  tree without the lint fix, passed everything except `lint`.

## [0.2.3-rc.1] - 2026-11-10

- **egui 0.32 -> 0.37 (eframe 0.37).** The toolkit moved through five breaking releases; the window
  code is unchanged in shape, but its wiring is not:
  - `eframe::App::update(&mut self, ctx, frame)` is now `App::ui(&mut self, ui, frame)`: the root `Ui`
    is the window, and the app keeps an owned `Context` for areas, overlays and textures.
  - `SidePanel`/`TopBottomPanel` were removed in 0.35. Every panel is now
    `egui::Panel::{left,right}(id).show(ui, ...)`, with `exact_width` -> `exact_size`,
    `default_width` -> `default_size` and `width_range` -> `size_range`.
  - `ctx.screen_rect()` -> `ctx.content_rect()` (14 sites, including the viewer's screen-anchored
    prev/next discs), `ctx.style()` -> `global_style()`, `Id::new` -> `Id::unique`,
    `TextEdit::frame(false)` -> `Frame::NONE`, and `ScrollArea`'s `State::offset()` ->
    `clamped_offset()`.
  - Screenshots use `Context::request_screenshot(callback)`. `ViewportCommand::Screenshot` carries a
    callback in 0.37 and `Event::Screenshot` is gone, so `--screenshot` saves and exits from the
    callback.
  - `default_fonts` is no longer a default feature; eframe still enables it here and `install_fonts`
    is unchanged.
  - Verified against rc.16 on the same live chat (`target/real_old_top.png` vs
    `target/real_egui37_top.png`) and on the preview screens (chat, Calls, Status, Channels,
    Communities, Media, Group info, Settings, viewer); the layout is identical.
- **A video with only its inline poster showed no play control.** The missing-file branch painted the
  embedded JPEG and stopped, so the bubble read as a photo until a tap opened the viewer. It now paints
  the same 48 px play disc as a downloaded video. GIFs are `kind == "video"` too and loop inline, so
  the disc is skipped when `message.gif` is set. The disc's icon box is 30 px so the triangle matches
  the official weight.
- Captures: `target/video_disc.png` (`WA_DESK_DEMO_VIDEO=1`) and `target/gif_no_disc.png` (a real GIF
  through `WA_DESK_GIFS`).
- **SQLite 3.54.1 for RUSTSEC-2026-0459 (CVE-2026-119375).** `cargo audit` found the bundled SQLite
  in `libsqlite3-sys` 0.37 vulnerable to uninitialised page-cache memory when a crafted `-wal`
  sibling is opened. Diesel 2.3.14 caps the crate at `<0.39.0` although its master allows `<0.40.0`;
  `vendor/diesel` carries that one-line widening and `vendor/whatsapp-rust-sqlite-storage` moves its
  own pin to 0.39. The lock resolves `libsqlite3-sys` 0.39.1 (SQLite 3.54.1); a live session opens
  the existing database and replays history.
- Tests: 132 native + 43 wa-core + 232 wa-ui + 7 installer/other, clippy `-D warnings` clean.

## [0.2.2-rc.16] - 2026-11-05

- **Opening a video could freeze the window.** The frame-texture sweep pushed every "protected" key
  (the playhead's own window) back onto the queue while it was still over the cap - an endless loop on
  the UI thread, which reads as a crash the moment a video opens. The sweep is bounded to one pass now;
  anything still over the cap simply re-decodes on a later frame.
- **Notifications are batched.** A chat that sent several lines in a row fired one Windows toast per
  message, and the toasts queued behind each other, so they arrived late. `notify_message` queues per
  chat and `flush_notifications` posts a single toast after 700 ms of quiet, reading
  `3 messages ? <latest line>` when it merged several. The existing gates (notifications off, status,
  newsletters, the open chat, focused window) are unchanged.
- Tests: 132 native + 43 wa-core + 232 wa-ui, clippy `-D warnings` clean.

## [0.2.2-rc.15] - 2026-11-05

- **A saved contact arriving from a LID no longer keeps its `~`.** Reported on "Test group" with
  "Yi Chi Fauzi": the row printed `~ Yi Chi Fauzi  +62 858-8205-3062` although the number is in the
  address book. `sender_row` decided "saved" from `sender_phone` only; a LID sender carries no PN in its
  JID, so when that field was absent the check fell back to the LID (never in an address book keyed by
  digits) and added the tilde. The check now also uses the number resolved for display, which is what the
  row shows anyway.
- Tests: 132 native + 43 wa-core + 232 wa-ui (the two `sender_row` tests still pass), clippy
  `-D warnings` clean.

## [0.2.2-rc.14] - 2026-11-05

- **The album grid is confirmed on real data.** `target/real_album_final.png` (`Test group`,
  `WA_DESK_SCROLL_TO=20`) shows one bubble holding three photos in a 2-column grid with a single clock.
  Getting there needed the last piece: `album_bubble` now takes the same `scroll` flag `bubble()` has and
  calls `scroll_to_rect` on its own rect, because the grid's height differs from the bubbles it replaced
  and the old index-based jump left it ~2,600 px off-screen (the trace that finally showed it:
  `album_bubble n=3 size=[264,283] top=2661`). The temporary magenta marker is gone.
- **Texture cache caps raised** (96 -> 256, stickers 240 -> 400): a scroll through a media-heavy chat was
  evicting textures still on screen, which re-decoded on the next frame and read as flicker while
  scrolling. Whether that is the whole story for the reported hover/scroll flicker is unconfirmed - the
  frame-pacing lines in `diagnostics.log` are the way to tell.
- Tests: 132 native + 43 wa-core + 232 wa-ui, clippy `-D warnings` clean.

## [0.2.2-rc.13] - 2026-11-05

- **A diagnostic for the album run** (`WA_DESK_DIAG=1`): the conversation now logs the run index, the span
  and the next three media messages (kind, sender, timestamp, side) whenever a media message does not
  become an album. It is how the real "Test group" data was read:
  `19 image ts=...086000 | 20 image ts=...197000 | 21 image ts=...197000` - so 20 and 21 share a timestamp and
  **should** group, while 19 is 111 s earlier.
- **What the trace shows, honestly**: the log fires only for index 19 (span 0, correctly - 19 to 20 is
  111 s), which means the loop never lands on 20 as a separate step: something already consumed 19-21.
  The capture at that moment still shows three full-width tiles, so the grid is either not drawn or drawn
  with the single-bubble fallback. This is unresolved, and the next step is to log *every* media index
  (the earlier attempt to relax the run-start condition failed on a quoting error and was not applied) and
  to log what `album_bubble` returns.
- **Scroll-wheel zoom** in the viewer: the wheel over the picture multiplies `viewer_zoom` (0.5-4x) in
  small steps, next to the toolbar magnifiers.
- **The prev/next discs no longer move when paging.** They were placed relative to the *stage*, which
  follows the picture, so a portrait photo after a landscape one slid them up and down; they are anchored
  to the window now (x at the screen edges, y at the screen's middle).
- Tests: 132 native + 43 wa-core + 232 wa-ui, clippy `-D warnings` clean.

## [0.2.2-rc.12] - 2026-11-05

- **`WA_DESK_SCROLL_TO` works now.** It is read where `WA_DESK_OPEN_CHAT` opens the chat (after the list
  exists) instead of at construction, so the target survives to the first conversation paint and beats
  the per-frame bottom pin. `target/real_album_test.png` now frames "Test group" index 20 instead of the
  bottom of the chat - the capture hook is finally usable for off-screen messages.
- **Album grid still not confirmed: the three real images did NOT collapse.** The framed messages are
  indices 20-22 from the same LID sender at the same/one-second timestamps
  (`20 ts=...197000, 21 ts=...197000, 22 ts=...198000`, all `from_me=false`, kind `image`), which satisfies
  `Model::album_span` on paper, yet they paint as three separate download bubbles. So either the model's
  `sender` values differ from the cached ones or the branch is skipped for another reason - the next step
  is a diagnostic line printing `span` and the two senders/timestamps for the first three images of a
  chat. Recorded as open, not as fixed.
- Tests: 132 native + 43 wa-core + 232 wa-ui, clippy `-D warnings` clean.

## [0.2.2-rc.11] - 2026-11-05

- **A capture hook for off-screen messages**: `WA_DESK_SCROLL_TO=<index>` frames one message the way a
  search hit does, and the list's per-frame "pin to the newest" is skipped for that frame
  (`suppress_bottom`), because the pin was undoing the jump.
- **Honest status: the hook does not fire yet.** It is read in `WaDesk::new`, so `scroll_target` is
  consumed on the first paint - before `WA_DESK_OPEN_CHAT` has loaded the chat asynchronously - and the
  conversation never sees it. `target/real_album_test.png` is still the bottom of "Test group"; the album
  grid remains unconfirmed on real data. The fix is to set the target where the chat-open hook applies
  (after the history request lands), not at construction.
- Tests: 132 native + 43 wa-core + 232 wa-ui, clippy `-D warnings` clean.

## Earlier releases (condensed)

- 0.2.2-rc.10 (2026-11-05) Viewer speaker and speed became real controls: mute repaints the glyph, and 0.5x/1x/1.5x/2x cycles through `Audio::set_speed` with the frames and scrubber following the same clock.
- 0.2.2-rc.9 (2026-11-05) Video viewer: the stage no longer swallows prev/next clicks, scrubbing paints the nearest decoded frame, and captions no longer cover the filmstrip.

- 0.2.2-rc.8 (2026-11-05) Viewer filmstrip: the strip now spans the stage width (centring only when the items fit)...
- 0.2.2-rc.7 (2026-11-05) The media viewer toolbar and zoom now follow the live client.
- 0.2.2-rc.6 (2026-11-05) A download button on media the phone has not sent.
- 0.2.2-rc.6 (2026-11-05) Albums are back.
- 0.2.2-rc.5 (2026-11-05) Channels' suggestion footer now has the live shape.
- 0.2.2-rc.4 (2026-11-05) Meta AI has a real entry point now, in the live rail position.
- 0.2.2-rc.3 (2026-11-05) The last two small live deltas are closed.
- 0.2.2-rc.2 (2026-11-05) The You drawer's status composer.
- 0.2.2-rc.1 (2026-11-05) The Media screen compared against the live panel.
- 0.2.1-rc.99 (2026-11-05) The download path is healthy; media problems now leave a trail by themselves.
- 0.2.1-rc.98 (2026-11-05) A download that never answers is retried instead of leaving the thumbnail forever.
- 0.2.1-rc.97 (2026-11-05) Newlines were swallowed, so numbered lists collapsed into one paragraph.
- 0.2.1-rc.96 (2026-11-05) The last two Communities deltas are closed.
- 0.2.1-rc.95 (2026-11-05) Communities compared against the live drawer and rebuilt in its shape.
- 0.2.1-rc.94 (2026-11-05) Channels finished against the live drawer.
- 0.2.1-rc.93 (2026-11-05) Channels compared against the live drawer, partially matched.
- 0.2.1-rc.92 (2026-11-05) The Status screen was compared against the live client and fixed row by row.
- 0.2.1-rc.91 (2026-11-05) Shared paint helpers moved to paint.rs.
- 0.2.1-rc.90 (2026-11-05) The window's buttons moved to buttons.rs.
- 0.2.1-rc.89 (2026-11-05) The icon vocabulary has its own module.
- 0.2.1-rc.88 (2026-11-05) Group info moved out of app.rs.
- 0.2.1-rc.87 (2026-11-05) Media viewer details, from the recorded tokens.
- 0.2.1-rc.86 (2026-11-05) The media viewer's still-image path is verifiable now.
- 0.2.1-rc.85 (2026-11-05) A group's header lists its participants, like the live client.
- 0.2.1-rc.84 (2026-11-05) Pinned and muted chats are visible, and stay that way.
- 0.2.1-rc.83 (2026-11-05) The empty conversation pane is the live one now.
- 0.2.1-rc.82 (2026-11-05) The chat list no longer deep-clones every chat per frame.
- 0.2.1-rc.81 (2026-11-05) The Settings drawer matches the live one's header, search and rows.
- 0.2.1-rc.80 (2026-11-05) The Group info rows are flat with leading glyphs, and Media carries a thumbnail strip.
- 0.2.1-rc.79 (2026-11-05) The Group info head now matches the live drawer.
- 0.2.1-rc.78 (2026-11-05) The Calls screen is the live drawer now.
- 0.2.1-rc.77 (2026-11-05) The chat-list filter row now matches the live sample.
- 0.2.1-rc.76 (2026-11-05) View-once bubbles exist now.
- 0.2.1-rc.75 (2026-11-05) The sticker panel's + opens the store sheet.
- 0.2.1-rc.74 (2026-11-05) The update button is the progress bar.
- 0.2.1-rc.73 (2026-11-05) Group info: the creator card.
- 0.2.1-rc.72 (2026-11-05) The recorded group-info deltas are closed (re-applied as four small verified edits after the
- 0.2.1-rc.71 (2026-11-05) The media probe was wrong, not the code.
- 0.2.1-rc.69 (2026-11-05) Video bubbles get a poster frame.
- 0.2.1-rc.68 (2026-11-05) Saved contacts no longer carry the ~ in group bubbles.
- 0.2.1-rc.67 (2026-11-05) Group info was read from the live pane (the user opened it) and its row list is now ours.
- 0.2.1-rc.66 (2026-11-05) The sticker + now lands somewhere useful.
- 0.2.1-rc.65 (2026-11-05) The sticker panel's recorded deltas are closed.
- 0.2.1-rc.64 (2026-11-05) The sticker panel was sampled from the live client.
- 0.2.1-rc.63 (2026-11-05) The two emoji-panel deltas from rc.62 are closed.
- 0.2.1-rc.62 (2026-11-05) The emoji panel was sampled from the live client with a real click (the browsermcp click...
- 0.2.1-rc.61 (2026-11-05) The composer has the official single smiley again.
- 0.2.1-rc.60 (2026-11-05) The "You" pane now matches the live drawer.
- 0.2.1-rc.59 (2026-11-05) Status sticker replies work end to end.
- 0.2.1-rc.58 (2026-11-05) The window itself left app.rs: src/app_impl.rs (470 lines).
- 0.2.1-rc.57 (2026-11-05) Image and video statuses render for real now, with the official blurred backdrop.
- 0.2.1-rc.56 (2026-11-05) The status viewer now carries the live chrome's author avatar and emoji control.
- 0.2.1-rc.55 (2026-11-05) Status replies, reactions and delete are wired - they existed in wa-core but nothing in t...
- 0.2.1-rc.54 (2026-11-05) Disappearing messages can be set for a group, for real.
- 0.2.1-rc.53 (2026-11-05) The Communities screen now matches the live drawer.
- 0.2.1-rc.52 (2026-11-05) The Media panel exists now, built from a live sample.
- 0.2.1-rc.51 (2026-11-05) The session glue left app.rs: src/session_glue.rs (299 lines).
- 0.2.1-rc.50 (2026-11-05) The QR gate left app.rs: src/gate.rs (489 lines).
- 0.2.1-rc.49 (2026-11-05) The pickers left app.rs: src/pickers.rs (782 lines).
- 0.2.1-rc.48 (2026-11-05) The menus and dialogs left app.rs: src/dialogs.rs (888 lines).
- 0.2.1-rc.47 (2026-11-05) The overlays left app.rs: src/overlays.rs (932 lines).
- 0.2.1-rc.46 (2026-11-05) The chat-list sidebar left app.rs: src/sidebar.rs (393 lines).
- 0.2.1-rc.45 (2026-11-05) The conversation left app.rs: src/conversation.rs (886 lines).
- 0.2.1-rc.44 (2026-11-05) src/screens/ gains the whole Settings screen.
- 0.2.1-rc.43 (2026-11-05) src/screens/ gains the Status screen.
- 0.2.1-rc.42 (2026-11-05) src/screens/ gains the Channels and Communities screens.
- 0.2.1-rc.41 (2026-11-05) app.rs split, module 7: src/screens/.
- 0.2.1-rc.40 (2026-11-05) Enter-to-send is verified end to end, not just by the unit test.
- 0.2.1-rc.39 (2026-11-05) The Notifications pane was sampled from the live client and rebuilt.
- 0.2.1-rc.38 (2026-11-05) A sticker you send now shows in the chat.
- 0.2.1-rc.37 (2026-11-05) Enter sends again, and a send that fails no longer pretends it worked.
- 0.2.1-rc.36 (2026-11-05) app.rs split, module 6: the bubble itself.
- 0.2.1-rc.35 (2026-11-05) Privacy and Chats were sampled from the live client and rebuilt to match.
- 0.2.1-rc.34 (2026-11-05) Media appears the moment it arrives, like the official client.
- 0.2.1-rc.33 (2026-11-05) WhatsApp markup renders instead of showing its asterisks.
- 0.2.1-rc.32 (2026-11-05) Sending shows the bubble at once.
- 0.2.1-rc.31 (2026-11-05) app.rs split, modules 4 and 5: src/settings_ui.rs and src/login_ui.rs.
- 0.2.1-rc.30 (2026-11-05) Status viewer header now matches the live sample.
- 0.2.1-rc.29 (2026-11-05) The official status viewer was sampled.
- 0.2.1-rc.28 (2026-11-05) A Settings sub-page was opened on the live client.
- 0.2.1-rc.27 (2026-11-05) docs/PARITY.md now documents the sampling recipe that unblocked the live work: the
- 0.2.1-rc.26 (2026-11-05) The attach menu was finally sampled from the live client - the blocker for four turns.
- 0.2.1-rc.25 (2026-11-05) app.rs split, third module: src/bubbles.rs.
- 0.2.1-rc.24 (2026-11-05) app.rs split, second module: src/rich_text.rs.
- 0.2.1-rc.23 (2026-11-05) Custom chat lists (the last ?
- 0.2.1-rc.22 (2026-11-05) Location messages, end to end.
- 0.2.1-rc.21 (2026-11-05) Received polls and contact cards render.
- 0.2.1-rc.20 (2026-11-05) Flicker when scrolling the chat list: the scrollbar width.
- 0.2.1-rc.19 (2026-11-05) Fixed the overlapping chat rows (the "seem broken" screenshot): a preview with newlines - a
- 0.2.1-rc.18 (2026-11-04) Bubbles no longer touch the window edge.
- 0.2.1-rc.17 (2026-11-04) Poll and contact bubbles render like the official client.
- 0.2.1-rc.16 (2026-11-04) The wire vocabulary for polls and contact cards.
- 0.2.1-rc.15 (2026-11-04) The composer's attach button opens the official menu instead of jumping straight to a file
- 0.2.1-rc.14 (2026-11-04) Status composer gained the official controls.
- 0.2.1-rc.13 (2026-11-04) Voice-note recording (was ?).
- 0.2.1-rc.12 (2026-11-04) Status list split into Recent and Viewed.
- 0.2.1-rc.11 (2026-11-04) Communities screen (was ?).
- 0.2.1-rc.10 (2026-11-04) Channels panel matches the official one.
- 0.2.1-rc.9 (2026-11-04) Settings list is the official one.
- 0.2.1-rc.8 (2026-11-04) Calls screen (missing feature).
- 0.2.1-rc.7 (2026-11-04) Status screen rebuilt to the official two-pane layout.
- 0.2.1-rc.6 (2026-11-04) Chat-list header is a 64 px bar in Palette::elevated (it used to be an 8 px-spaced row of
- 0.2.1-rc.5 (2026-11-04) Animated WebP stickers actually animate now.
- 0.2.1-rc.4 (2026-11-04) Composer matches the official bar.
- 0.2.1-rc.3 (2026-11-04) CI fix: spawn::quiet kept a mut command that only Windows uses, so the Linux lint job
- 0.2.1-rc.2 (2026-11-04) Emoji in bubbles are the official size again, with padding.
- 0.2.1-rc.1 (2026-11-04) The chat has the official wallpaper again.
- 0.2.0-rc.102 (2026-11-04) Memory: the last unbounded caches are capped, with a diagnostic to prove it.
- 0.2.0-rc.101 (2026-11-04) Fixed a deadlock in the texture cache.
- 0.2.0-rc.100 (2026-11-04) Media viewer rebuilt against the official screen: the sender's avatar, name and
- 0.2.0-rc.99 (2026-11-04) Media bubble padding polished.
- 0.2.0-rc.98 (2026-11-04) Photos and videos sit inside the bubble now.
- 0.2.0-rc.97 (2026-11-04) Photos and videos that cannot download now say so.
- 0.2.0-rc.96 (2026-11-04) Media loads instantly on the next launch.
- 0.2.0-rc.95 (2026-11-04) The Packs tab is real now.
- 0.2.0-rc.94 (2026-11-04) Group bubbles have the sender's photo in the left gutter (the request from the official
- 0.2.0-rc.93 (2026-11-04) Date chips separate days in a conversation.
- 0.2.0-rc.92 (2026-11-04) Adaptive echo cancellation replaces ducking as the first line of defence.
- 0.2.0-rc.91 (2026-11-04) Bubbles tail like the official client.
- 0.2.0-rc.90 (2026-11-04) GIFs animate in the bubble.
- 0.2.0-rc.89 (2026-11-04) Call echo suppression.
- 0.2.0-rc.88 (2026-11-04) The Create tile makes stickers.
- 0.2.0-rc.87 (2026-11-04) The GIF tab works (it used to say "not supported yet").
- 0.2.0-rc.86 (2026-11-04) Outgoing call video works: the webcam is captured, encoded and sent, so a video call is now
- 0.2.0-rc.85 (2026-11-04) Remote call video is decoded and painted.
- 0.2.0-rc.84 (2026-11-04) The sticker picker is the official panel (it was still the plain window with two stacked
- 0.2.0-rc.83 (2026-11-04) Read receipts (the checkmarks) are back.
- 0.2.0-rc.82 (2026-11-04) The AI auto-reply loop is back (it only existed in the removed JS shell).
- 0.2.0-rc.81 (2026-11-04) Desktop notifications (JS-only since the migration).
- 0.2.0-rc.80 (2026-11-04) Chinese, Japanese and Korean text renders with real glyphs.
- 0.2.0-rc.79 (2026-11-04) Videos play in the media viewer (they only showed a poster after the migration).
- 0.2.0-rc.78 (2026-11-04) CI and the Linux build notes install ALSA headers (libasound2-dev / alsa-lib-devel), which
- 0.2.0-rc.77 (2026-11-04) Voice notes and audio messages play (JS-only after the migration).
- 0.2.0-rc.76 (2026-11-04) Settings rebuilt like the official screen: profile header (photo, name, connection), the
- [0.2.0-rc.58] — 2026-11-04 () Documentation now matches the code: no Tauri, no WebView, no unused libraries.
- [0.2.0-rc.57] — 2026-11-04 () The WebView front end is gone: the app is now Rust all the way down.
- [0.2.0-rc.56] — 2026-11-03 () The native window has a Settings screen.
- [0.2.0-rc.55] — 2026-11-03 () The native window lists followed channels.
- [0.2.0-rc.54] — 2026-11-03 () The native window shows status/stories.
- [0.2.0-rc.53] — 2026-11-03 () The native window shows media.
- [0.2.0-rc.52] — 2026-11-03 () The native window links an account: QR, pairing code and status.
- [0.2.0-rc.51] — 2026-11-03 () A native front end exists: apps/desktop-native (binary wa-desk-native).
- [0.2.0-rc.50] — 2026-11-03 () new-chat.js and settings.js are folded into chat-list.js - 15 ?
- [0.2.0-rc.49] — 2026-11-03 () media-viewer.js and main.js are gone - 17 ?
- [0.2.0-rc.48] — 2026-11-03 () chat-menu.js + message-actions.js are gone - merged into one menus.js
- [0.2.0-rc.47] — 2026-11-03 () emoji.js is deleted - 19 ?
- [0.2.0-rc.46] — 2026-11-03 () attachments.js is deleted - 20 ?
- [0.2.0-rc.45] — 2026-11-03 () contacts.js is deleted - 21 ?
- [0.2.0-rc.44] — 2026-11-03 () group-info.js is deleted - 22 ?
- [0.2.0-rc.43] — 2026-11-03 () chat-search.js is deleted - one fewer JS file (23 ?
- [0.2.0-rc.42] — 2026-11-03 () The contact-plan views and the sent-echo fold moved to Rust.
- [0.2.0-rc.41] — 2026-11-03 () Star, mute and the contact-sync gate moved to Rust.
- [0.2.0-rc.40] — 2026-11-03 () The status composer's draft rules moved to Rust (wa_ui::status).
- [0.2.0-rc.39] — 2026-11-03 () The history re-sync rules moved to Rust (wa_ui::history).
- [0.2.0-rc.38] — 2026-11-03 () The group-screen rules moved to Rust.
- [0.2.0-rc.37] — 2026-11-03 () Reactions moved to Rust (wa_ui::reactions).
- [0.2.0-rc.36] — 2026-11-03 () The chat scroll bookkeeping moved to Rust.
- [0.2.0-rc.35] — 2026-11-03 () The story viewer's chrome state moved to Rust.
- [0.2.0-rc.34] — 2026-11-03 () Dead-code sweep with a new audit.
- [0.2.0-rc.33] — 2026-11-03 () The document file-card helpers moved to Rust (and the dead ones are gone).
- [0.2.0-rc.32] — 2026-11-03 () Fixed: the delete dialog never opened.
- [0.2.0-rc.31] — 2026-11-03 () The AI settings form is validated in Rust.
- [0.2.0-rc.30] — 2026-11-03 () The chat merge and the optimistic text bubble moved to Rust.
- [0.2.0-rc.29] — 2026-11-03 () The chat-row store mutation moved to Rust.
- [0.2.0-rc.28] — 2026-11-03 () Three more undefined-handler crashes fixed.
- [0.2.0-rc.27] — 2026-11-03 () Fixed: "Select chats" threw a ReferenceError.
- [0.2.0-rc.26] — 2026-11-03 () The profile-photo pump, the theme rules and the avatar/media icon tables
- [0.2.0-rc.25] — 2026-11-03 () The call codec and audio constants moved to Rust.
- [0.2.0-rc.24] — 2026-11-03 () The composer's media-send pipeline is planned in Rust.
- [0.2.0-rc.23] — 2026-11-03 () Media-hydration decisions moved to Rust.
- [0.2.0-rc.22] — 2026-11-03 () Status view receipts and interaction healing moved to Rust.
- [0.2.0-rc.21] — 2026-11-03 () The status viewer's group queue is planned in Rust.
- [0.2.0-rc.20] — 2026-11-03 () The call overlay is driven by Rust.
- [0.2.0-rc.19] — 2026-11-03 () The disk chat-list mirror and the address-book renames are Rust decisions.
- [0.2.0-rc.18] — 2026-11-03 () AI context and sticker selection moved to Rust.
- [0.2.0-rc.17] — 2026-11-03 () Composer, seeding and peer-alias plans moved to Rust.
- [0.2.0-rc.16] — 2026-11-03 () Chat-menu actions are planned in Rust.
- [0.2.0-rc.15] — 2026-11-03 () The story viewer's frame clock moved to Rust.
- [0.2.0-rc.14] — 2026-11-03 () Incoming-message application rules moved to Rust.
- [0.2.0-rc.13] — 2026-11-03 () Attachment validation and in-chat search planning moved to Rust.
- [0.2.0-rc.12] — 2026-11-03 () icons.js and chat-utils.js are gone.
- [0.2.0-rc.11] — 2026-11-03 () Status and message actions are guarded by Rust.
- [0.2.0-rc.10] — 2026-11-03 () The remaining store flows moved to Rust.
- [0.2.0-rc.9] — 2026-11-03 () The webview cache layer moved to Rust.
- [0.2.0-rc.8] — 2026-11-03 () Chat-store housekeeping moved to Rust.
- [0.2.0-rc.7] — 2026-11-03 () The status composer is driven by Rust.
- [0.2.0-rc.6] — 2026-11-03 () The chat header comes from the renderer.
- [0.2.0-rc.5] — 2026-11-03 () The media viewer is rendered by Rust.
- [0.2.0-rc.4] — 2026-11-03 () The Settings screen is rendered by Rust.
- [0.2.0-rc.3] — 2026-11-03 () The status/stories screen is rendered by Rust.
- [0.2.0-rc.2] — 2026-11-03 () The UI model moved into Rust.
- [0.1.0-rc.6] — 2026-11-03 () The update channel now works for pre-releases.
- [0.1.0-rc.5] — 2026-11-03 () In-app updates.
- [0.1.0-rc.4] — 2026-11-03 () The window now opens at a size that fits the screen.
- [0.1.0-rc.3] — 2026-11-02 () The Account screen shows the real app version.
- [0.1.0-rc.2] — 2026-11-02 () aes-gcm 0.10 ?
- [0.1.0-rc.1] — 2026-11-02 () The app version is 0.1.0-rc.1.
