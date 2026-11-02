# Changelog

Newest first. Each entry keeps the mechanism (files, protocol constants, test
names) and the verification signal. Durable design notes, the feature inventory
and the roadmap live in [`AGENTS.md`](AGENTS.md).

- **Bundle identifier is now `io.github.dirttw.wadesk`** (was the placeholder
  `dev.example.wadesk`), so the installer's `Manufacturer` reads `dirttw`, the
  MSI `UpgradeCode` is regenerated from the real id, and the app data moves to
  `%APPDATA%\io.github.dirttw.wadesk` (Linux:
  `~/.local/share/io.github.dirttw.wadesk`). Updated in `tauri.conf.json`, the
  Windows toast `AUMID` (`notifications.rs`), the portable `README.txt`
  (`build-portable.bat`), `README.md` and `AGENTS.md`. An existing linked
  session keeps working by moving the old data folder to the new name.
- **Windows installers (MSI + NSIS) with uninstallers, and a PII-free first
  commit.** `npx @tauri-apps/cli build` (Tauri CLI 2.12.4) produces
  `bundle/msi/WA Desk_0.1.0_x64_en-US.msi` and
  `bundle/nsis/WA Desk_0.1.0_x64-setup.exe`. The MSI registers
  `ProductCode`/`UpgradeCode` with `ALLUSERS=1`, so it uninstalls from
  Settings → Apps and upgrades in place; the NSIS setup ships its own
  `uninstall.exe`.
  - `tauri.conf.json` gained `bundle.resources` mapping the repo `LICENSE.md`
    into the install dir, so the AGPL text travels with the binary. Verified by
    extracting the MSI (`msiexec /a`): it installs `wa-desktop.exe` plus
    `PFiles\WA Desk\LICENSE.md`, with MSI properties
    `ProductName=WA Desk`, `ProductVersion=0.1.0`, `ALLUSERS=1`.
  - Every personal identifier was redacted before the first commit: the
    self-chat handle, contact names, channel name, group ids and phone numbers
    in docs, comments and test fixtures now use `Alice`/`Bob`,
    `6281234567890`, `120363000000000001`, `111111111111111`, and
    `scripts/wadesk-find-wajs.mjs` reads `%LOCALAPPDATA%` instead of a
    hard-coded user path. Verified with `git grep --cached` (no matches).
    `crates/wa-core` tests still pass (34/34) and the commit author uses the
    GitHub noreply email.
- **README reorganized and given a development note.** The public README now
  opens with the description and disclaimer, then a **Development note** stating
  the project is 100% vibe-coded (the author does not write code), crediting the
  coding models (**DeepSeek V4.1 Flash**, **GLM 5.3 Flash**), and setting honest
  expectations (no security audit, handles encrypted data, use a secondary
  account). A short **What it does** highlights section was added, and the
  build/run sections were moved above the licence material so the reader reaches
  the practical steps first; the licence table, vendored/runtime component lists
  and **Why AGPL?** are unchanged in content and now live under `## License`.
- **Relicensed to AGPL-3.0-only (the binary links AGPL code).** `whatsapp-rust`'s
  `wacore` depends on `wacore-libsignal` without `optional = true`, and that crate
  declares `license = "MIT"` while **31 of its files carry Signal Messenger's
  `SPDX-License-Identifier: AGPL-3.0-only` header** (no MIT headers, no `LICENSE`
  file), so `cargo`/crates.io/`cargo-deny` misreport it as MIT (upstream
  [`oxidezap/whatsapp-rust#1828`](https://github.com/oxidezap/whatsapp-rust/issues/1828)).
  Rust statically links it into the exe
  (`wacore-libsignal → wacore → whatsapp-rust → wa-core → wa-desktop`), making the
  distributed binary a combined work under the AGPL. Fixed by:
  - `[workspace.package] license = "AGPL-3.0-only"` in `Cargo.toml` (propagates to
    `wa-types`, `wa-core`, `wa-desktop`);
  - new [`LICENSE.md`](LICENSE.md) with the verbatim AGPL-3.0 text;
  - `README.md` — the "Used libraries and licenses" section now leads with the AGPL
    statement, lists `wacore-libsignal` as `AGPL-3.0-only`, and a **Why AGPL?**
    section records the evidence and the upgrade path (drop the crate or wait for
    the metadata fix to relicense back).
  - Verified: the crate is in `Cargo.lock` (`wacore-libsignal 0.7.0`), the compiled
    `wacore_libsignal-*.d` lists `curve25519.rs`/`session_cipher.rs`/`ratchet`/
    `group_cipher`, and `dist\WA Desk.exe` contains `libsignal` ×125, `sender_key`
    ×90, `ratchet` ×15, `curve25519` ×12.
- **The reply preview now sits above the composer, matching the official.** The
  banner used to render *below* the text box: `#reply-banner` was placed after
  `<form id="composer">` in `index.html`, so the flex column stacked it under the
  input (the CSS comment already said "above the composer" — the DOM order was
  the bug). The draft preview, the edit banner and the reply banner now come
  before the composer, in that order, so the stack is
  `draft → edit-banner → reply-banner → composer`.
  - Styling measured from the live official client (a reply open in
    web.whatsapp.com, read with `getComputedStyle`): the banner is an inset
    rounded card — 12 px from the panel edges, same width as the composer box —
    with `background: #242626` (`--WDS-surface-elevated-emphasized`), a **4 px**
    left bar and the author name in **the quoted sender's colour** (the official
    showed the sender as `#53BDEB`, the same colour it gives their name in the message
    list), the name at 12.8 px/500 and the preview in `rgba(255,255,255,.6)`.
    `startReply` now sets `--reply-colour` from `WAChatUtils.senderColor(sender)`;
    `.reply-banner`/`.edit-banner`/`.draft` are inset cards, and the composer lost
    its `border-top` (the official footer has none — the `#242626` box separates
    it).
  - Verified in our client with a reply open (a group chat, replying to
    a message): DOM order
    `… > draft > edit-banner > reply-banner > composer`, computed
    `background: rgb(36,38,38)`, `border-left: rgb(255,114,161) 4px`,
    `margin: 6px 12px 4px`, name colour `rgb(255,114,161)`; pixel scan of the
    capture shows the card at x=404 (12 px inside the panel) with a 4 px
    `#53A6FD` bar and a `#242626` body — the same geometry and colours as the
    official.
  - **The scroll-to-latest disc lifts with the banner.** It was pinned at
    `bottom: 84px` (tuned for the bare composer), so the banner put it straight
    on top of the ✕ — the ✕ was unreachable. `initScrollLatestOffset()`
    (`chat-render.js`, called from `wire()`) watches the composer and the three
    cards with one `ResizeObserver` and sets the button's `bottom` to the
    measured stack height + 19 px, so it moves up when a card appears and back
    down when it is dismissed — no scroll-time layout reads. Verified live: with
    a reply open `bottom` went `84px → 147px`, the disc rect was
    `y 628–661` against the banner's `y 689–741` (36 px clear of the ✕,
    `overlaps: false`), `document.elementFromPoint` at the ✕'s centre returned
    `reply-cancel`, and dismissing the reply returned `bottom` to `84px`.
- **The colour theme now matches the shipped client exactly.** The palette was
  taken from the official design tokens (`--WDS-*`) read live out of
  web.whatsapp.com (`getComputedStyle` on the real page, via the browser MCP;
  the stylesheet itself is `static.whatsapp.net/rsrc.php/v5/y7/l/0,cross/…css`,
  see `scripts/waweb-fetch-css.mjs` / `waweb-theme-tokens.mjs`), and every
  rendered element was verified by capturing both windows and sampling pixels
  (`scripts/win-capture.ps1` / `win-sample.ps1` / `win-scan.ps1` — the capture
  path was validated by checking it reproduces our own known CSS values exactly).
  - Dark theme: surface `#161717` (sidebar, chat, headers), rail `#1D1F1F`,
    elevated `#242626` (search/composer/incoming bubble), hover & selected rows
    `rgba(255,255,255,.1)` → `#2E2F2F`, divider `rgba(255,255,255,.1)`, text
    `#FAFAFA`, secondary `rgba(255,255,255,.6)`, disabled `#424445`, accent
    `#21C063`, accent-deemphasized (active filter pill) `#103529`,
    accent-emphasized `#D9FDD3`, outgoing bubble `#144D37`, links `#21C063`
    (green, not blue), read ticks `#53BDEB`, danger `#FB5061`, on-accent
    `#0A0A0A`, outline `#757778`, app wash `#262524`.
  - Light theme: surface `#FFFFFF`, rail `#F7F5F3`, wallpaper `#F5F1EB`,
    accent `#1DAA61`, pill selected `#D9FDD3`/`#15603E`, outgoing bubble
    `#D9FDD3`, link `#1B8755`, read `#007BFC`, danger `#EA0038`.
  - Group sender colours and default avatars switched to the official
    `--WDS-components-profile-photo-content-*` / `-surface-*` palettes (avatar =
    dark tinted circle + its matching letter colour).
  - The compose button is the official accent disc (`#21C063` with a `#0A0A0A`
    glyph), the filter pills are transparent-with-a-border and the active one
    uses `#103529`/`#D9FDD3`, and unread rows no longer tint their timestamp
    green (the official keeps it `rgba(255,255,255,.6)`).
  - Legacy overlays were re-based on the official palette
    (`rgba(11,20,26,·)`→`rgba(0,0,0,·)`, `rgba(134,150,160,·)`→`rgba(255,255,255,·)`,
    `#00a884`→`#21C063`, `#04231c`→`#0A0A0A`, `#005c4b`→`#144D37`).
  - Verified by pixel comparison, ours vs the live official window:
    `#161717 / #1D1F1F / #161717 / #161717 / #2E2F2F / #2E2F2F / #242626` for
    panel, rail, chat, header, search, selected row, incoming bubble — identical
    in both; FAB `#21C063`; outgoing bubble `#144D37`; light theme
    `#FFFFFF / #F7F5F3 / #F5F1EB / #D9FDD3`.
  - Secondary screens verified against the official tokens (computed styles via
    CDP): every screen is `--WDS-surface-default` `#161717` (settings, new chat,
    community, group info, channels, starred, emoji panel), every popover/menu is
    `--WDS-surface-elevated-emphasized` `#242626`, the media viewer backdrop is
    `#161717` (sampled from the official viewer), and the **status viewer stage is
    `#0B141A`** — that is the official `--status-background-rgb` (`11, 20, 26`),
    which our viewer previously painted pure black. The call overlay scrim is the
    neutral `rgba(0,0,0,.92)`.
  - The status text composer palette is now the official one: the shipped
    client's "Palette" button repaints the composer from the 20-colour array in
    its bundle (`#A62C71 #90A841 #C1A03F #792138 #AE8774 #F0B330 #B6B327 #C69FCC
    #8B6990 #FF8A8C #54C265 #FF7B6B #26C4DC #57C9FF #74676A #7E90A3 #5696FF
    #6E257E #7ACBA5 #243640`), replacing our hand-picked 16.
  - An audit script (`scripts/_theme-audit.mjs` run) checks every colour literal
    in the UI against the 1,222 token values parsed out of the official
    stylesheet: the only literals left are document/file-type icon colours
    (Word/Excel/PDF badges), HTML entities that look like hex, and alpha variants
    of official hexes — no off-palette chrome.
- **The phone's address book now syncs (the `critical_unblock_low` snapshot MAC
  mismatch).** A fresh pairing used to show push names (`Device Name`,
  `Contact Name`) instead of the saved ones; the address book was delivered and
  thrown away because the snapshot's aggregate MAC never verified
  (`297 records`, `computed=c8557bc5… expected=2741568c…`).
  - **The hash algorithm was not the bug.** The vendored code was checked against
    the shipped client's own bundle (script URL recovered from the WebView2 code
    cache, see `scripts/wadesk-find-wajs.mjs` / `wadesk-bundle-win.mjs`):
    `WACryptoLtHash` = HKDF-SHA256 info `WhatsApp Patch Integrity`, 128-byte zero
    `EMPTY_LT_HASH`, LE u16 lanes, 128-byte keys; snapshot elements are
    `value.blob.slice(-32)` **deduped by `index.blob`**; `generateSnapshotMac` =
    `HMAC-SHA256(snapshotMacKey, ltHash ‖ version_BE64 ‖ collection)`. Exactly what
    the library computes (whatsmeow/Baileys agree), and no stored key (all 72 rows)
    or algorithm variant reproduces the server's `mac` — it does not describe the
    record set served to this companion.
  - **The shipped client doesn't drop the collection either**: on mismatch it runs
    `WAWebSyncdAntiTamperingLtHash.checkLtHash` (scratch ltHash from the local DB)
    and latches `isCollectionInMacMismatchFatal`, after which the aggregate
    comparison is skipped. The library already did this for authentic-but-diverged
    *patches*; snapshots were the one path that hard-failed.
  - **Fix** (`vendor/wacore-appstate/src/processor.rs`, `process_snapshot`): on an
    aggregate mismatch, log it, set `HashState::mac_mismatch_fatal` (so later
    patches skip the aggregate too) and **apply the records anyway** — each record
    stays authenticated by its own content MAC, so tampering still fails. A
    verified snapshot clears the flag. Locked by
    `snapshot_with_diverged_aggregate_mac_still_applies_authentic_records` and
    `snapshot_with_tampered_record_mac_is_still_rejected` (77/77 tests).
  - The crate is vendored + `[patch.crates-io]` but *excluded* from the workspace
    (its `simd` default needs nightly): test it with
    `cd vendor/wacore-appstate; cargo test --no-default-features`.
  - Verified live: `contacts.json` 20542 bytes / **590** name entries (LID + PN per
    contact), UI `contacts: 590`, sidebar reads `Alice`, `Bob`,
    `Carol`, `Dave`.
- **Pairing: the QR no longer disappears, and it counts down in seconds before
  rotating.** Three bugs made the login screen dead:
  - A stale device store left no code at all: `Client::logout` left credentials on
    disk, the next start replayed them, the server answered **401 LoggedOut** and
    nothing emitted a QR. The pump now owns teardown (`UiEvent::LoggedOut` →
    `state::restart_after_logout()`).
  - The wipe failed with `os error 32` (vendored workers keep an `Arc<Client>`
    alive past `shutdown()`, and it owns the SQLite file). `wa-core`'s deferred
    app-state retries (delete-for-me, pin) now sleep through a `Stopper`
    (`AtomicBool` + `Notify`) tripped by `Session::shutdown()`, and when the unlink
    still fails the logout **bumps the store generation** (`store.gen`,
    `whatsapp.db` → `whatsapp-1.db`) and opens the next session on the fresh file;
    `purge_stale_stores` cleans the dead one at the next start.
  - The countdown/rotation were invisible: `#qr-timer` + `#qr-hint` tick every
    250 ms from the vendor's `timeout_secs`; `PairingQrCodesExhausted` maps to
    `UiEvent::QrExpired` → "The pairing code expired." + a **New code** button
    (`restart_session`). Verified live: counter 59→9s, rotation, and recovery.
- **A second copy of the app now says so instead of crashing.** Two instances
  panicked with `WebView2 error 0x8007139F` (shared user-data folder + single-owner
  store); `claim_single_instance()` takes `Local\wa-desk-single-instance` before the
  builder and shows a message box.
- **Status replies/reactions render with WhatsApp's "· Status" card.** A status
  interaction is marked by `contextInfo.remoteJid == status@broadcast`, which
  nothing carried through: the reaction rendered as a squeezed plain reply *and*
  the reply-quote recovery re-emitted the status as its own bubble.
  `MessageView.status_quote` + `util::quotes_status()` (set in `events.rs`, recovery
  skips status quotes); UI draws `statusQuoteHtml()`; `healStatusInteractions()`
  fixes old caches; `reply_status` now quotes the status too. Locked by
  `status_interactions_are_marked_for_the_status_card` /
  `status_replies_quote_the_status`.
- **Video posters are real sampled frames, not the blurry inline JPEG.**
  `captureVideoPoster` bailed out on `state.thumbs.has(id)`, so the 72×72 inline
  thumb always won; sampled frames now live in `state.videoPosters` (LRU 120),
  `thumbOf()` prefers them, `ensureVideoPoster` runs on the `<video>` path, and
  frames are sampled at 480 px (measured `576×1024` → **480×853** frame vs 72×72).
- **Chat photos and videos take the shape of the media.** A video element sizes
  itself from its poster, so a 576×1024 clip rendered in a 72×72 box;
  `media-ui.js`'s `loadedmetadata` now reshapes the wrapper for chats (fits the clip
  inside 420 px, narrows for portrait), and `.media-image img` / `.media-video`
  use `width: auto` + `max-width: 100%` + `max-height: 420px` so the ratio survives
  and a tall photo is no longer cropped. Placeholder tile 330×186; `.album-cell
  .media-box` pins `min-width/min-height: 0`.
- **Group bubbles carry the sender's photo and match the official layout.** 28 px
  `.avatar.msg-avatar` in a left gutter (`.messages.group-view { padding-left:
  52px }`, photo `left: -36px`) once per sender run; `senderHtml()` now gates name +
  photo + tail on `firstOfRun`; the `✆ +62 …` phone row is gone
  (`senderDisplayName()`); `patchMessageAvatar()` swaps initials for a late photo.
  Group chats only.
- **Status viewer: smooth bars, no black line, working pause glyph.** The pause
  glyph was `hidden`-attributed and lost to `[hidden] { display: none !important }`
  (class swap is now authoritative); the active segment was painted only by
  `paintStatusProgress`, so videos-before-metadata / the `fetch_media` window /
  the unavailable branch left a `71,71,71` empty track (the frame now anchors its
  clock on render, 20 s placeholder for video, 3 s on failure, late callbacks
  ignored by a per-frame token); and the bar animated `width` through a CSS
  transition (now a `transform: scaleX()` layer on one rAF loop — 120/120 frames
  distinct, monotonic, 5.3–5.8 ms pacing).
- **The sidebar kebab opens the official chat-list menu.** `#side-menu` = New
  group, Starred messages, Select chats, Mark all as read, divider, App lock
  (honest "not supported" toast), Log out. Starring rides the `star` app-state
  collection (`Session::star_message`, `Event::StarUpdate` → `chat_star`,
  `#starred-screen`; locked by `star_updates_from_other_clients_reach_the_ui`).
  Select chats (`#chat-select-bar`) is a real multi-select with Mark as read /
  Archive / Delete; Mark all as read walks `chat_read_mark`.
- **Toasts show as "WA Desk", not "Windows PowerShell".** Unpackaged exes must
  register an AUMID under `HKCU\Software\Classes\AppUserModelId\<id>` with a
  `DisplayName`; `register_aumid()` writes it at startup (`reg add`,
  `CREATE_NO_WINDOW`, optional `IconUri`). A Start-menu/`IPropertyStore` attempt
  heap-corrupted the process (`0xc0000374`) and was replaced by the registry route.
- **Notifications play a sound.** The Rust toast gate also plays
  `PlaySoundW("Notification.Default", SND_ASYNC | SND_SYSTEM)`, so background
  chats ping even hidden to the tray and per-app toast-sound settings can't silence
  it.
- **Windows push notifications fire from the Rust event pump.** The old JS
  `sendNotification` path died whenever the webview was hidden/reloading/throttled;
  `notifications.rs` gates every `UiEvent::Message` (own sends, open chat, muted
  chats, the preference) and toasts it. The webview only mirrors what Rust cannot
  see via `set_notify_state` (`scheduleNotifySync`), and `test_notify` pushes a
  sample through the real `event_tx`.
- **Refactor/clean-up pass.** Clippy clean across `wa-core`/`wa-types`/`wa-desktop`;
  `scripts/find-unused.mjs` cross-checks every top-level declaration (zero dead);
  `updateRailBadges` early-returns on an unchanged computed key.
- **Communities open the official community screen.** `#community-screen` (Back +
  name, **Announcements**, **Groups you're in** + **Add group**, children with
  previews) via `openCommunity`/`closeCommunity`/`renderCommunityScreen` in
  `chat-list.js`; Esc/Back return to the list.
- **Communities render like the official chat list.** Community parents never
  appear in history sync and the flat activity sort buried them; `GroupInfoView`
  now carries `is_community` / `parent_jid` / `is_announce`, `chat-list.js` fetches
  group metadata lazily (~900 ms, cached in `state.groupMeta`), parents render as
  headers with children nested and indented, the announcement subgroup is hidden,
  and a parent is seeded on demand.
- **Rail notification badges.** Unread-chat count pill on the chats icon (hidden at
  zero) and a dot on the status icon; `updateRailBadges()`/`setRailBadge()` are
  called from **both** exits of `renderChatList` (a first attempt landed in the
  channels function and never fired on incoming mail), `status.js` reports via
  `setRailStatusUnseen()`.
- **The push-name leak that renamed groups ("Alice🧢").** A view-once fanout stored
  the sender's push name under the **group's** id (the fanout arm lacked the
  `!is_group` guard the live arm had); both insert sites fixed, locked by
  `fanout_push_names_never_rename_groups`, and `Session::group_info` re-emits the
  subject as `ChatName` so stale names heal on fetch.
- **Every icon is Google Material Symbols.** All 127 inline SVGs swapped for
  Material artwork (`gen-material-icons.mjs` → `ui/icons.js` `waIcon()`,
  `apply-material-icons.mjs` swaps paths while keeping classes/ids, `count-icons.mjs`
  verifies 0 legacy). Artwork is on the −960…960 grid (`viewBox="0 -960 960 960"`);
  ticks/play/chevrons/theme glyphs became icons too. Four CSS rules that styled
  icons as stroked outlines (`.media-nav svg`, `.status-add svg`,
  `#status-text-emoji/#status-text-colour svg`, `.status-privacy svg`) were switched
  to fill-based.
- **View-once placeholder renders the official compact one-liner.** The 5-line
  block came from the 320 px body cap, media width caps, and `white-space: pre-wrap`
  rendering template indentation; body is now block-flow 12.5px italic, bubbles hug
  one line, `has-media` dropped, "Tap to view" is a hover tooltip (measured
  587×24 body vs the old 320×139).
- **View-once: the recovered copy becomes a kept bubble.** When anyone replies, the
  quoting client embeds the original body (view-once wrapper + original media key
  and direct path) in `contextInfo.quotedMessage`; `events.rs` caches that copy and
  emits a normal image/video bubble above the reply, which the UI merges in place
  over the placeholder and downloads through the normal pipeline. Locked by
  `reply_quotes_recover_view_once_media`. A fanout arriving as only
  `<unavailable type="view_once">` stays a placeholder (the phone refuses resends).
- **View-once recovered copies render as a normal media preview.** `once_recovered`
  drives `chat-render.js`: unrecovered → `viewOncePlaceholderBody()` (clock icon,
  dim text, FAQ anchor with `target="_blank"` — without it WebView2 hijacked the
  window); recovered → hidden placeholder with the rail eye off, kept media +
  caption with the eye on, and directly openable via `data-view-once-open`. The
  recovered copy keeps the **original** message's side (`contextInfo.participant`;
  in DMs the quote belongs to the opposite side). Locked by
  `own_reply_keeps_the_recovered_view_once_on_the_quoted_side` /
  `direct_reply_without_participant_quotes_the_peer`.
- **Pin is instant from every client.** `Session::pin_chat` sends the optimistic
  `UiEvent::ChatPin` immediately and runs the patch in a spawned task with conflict
  retries (10 s/30 s/90 s/240 s) and a revert event; the old `tokio::spawn` from the
  sync command thread crashed on racing pins. Menu code moved to `chat-menu.js`
  (`initChatMenu()`); `Event::PinUpdate` → `chat_pin` locked by
  `pin_updates_from_other_clients_reach_the_ui`.
- **Chat pin + the full official right-click chat menu.** Archive, Mute› (8 h/1 week/
  Always/Unmute), Pin↔Unpin, Mark read↔unread, Add to list (disabled), Block, Close,
  Clear, Delete — each riding `Session::archive_chat/mute_chat/mark_chat_unread/
  delete_chat/clear_chat/block_contact`, with `ArchiveUpdate`/`MuteUpdate`/
  `MarkChatAsReadUpdate` folded in `events.rs`. Archived chats get an "Archived"
  toggle row, pinned chats float to the top with a pin glyph, mute hides the bell
  and silences notifications.
  Official reference (`WAWebPinChatSync`): `CollectionName.RegularLow`,
  `getVersion() === 5`, index `[Actions.Pin, chatJidIndex1]`, value
  `{pinAction:{pinned}}`, plus a companion unarchive mutation when pinning; the
  vendored `schemas::PIN` matches and the index JID resolves through the chat's
  `accountLid` (LID-aware caches cover it).
- **Read receipts reach our client (blue double tick).** Receipts are narrated from
  the **peer's LID** while chats/messages are keyed by the **phone-number JID**, so
  `Event::Receipt` never matched: `canonical()` (`events.rs`) now falls back through
  the alias map when the LID chat doesn't exist, and the webview no longer drops
  unresolved receipt/`message_deleted`/`message_edited` (ids are unique, so it scans
  the chat map). Locked by `chat_read_receipt_moves_our_sent_state`.
- **Chat-list previews follow the official rules** (`chat-utils.previewOf`): a DM
  shows only the message, own DMs get a separate tick (`previewTick`: grey ✓,
  grey ✓✓ delivered, blue ✓✓ read/played), groups keep `Author: `/`You: ` and the
  unsaved `~` marker. Covered by `scripts/test-chat-utils.js`.
- **Delete-for-me survives the post-startup app-state conflict window.** A delete
  right after launch could fail with "app-state patch for regular_high still
  conflicting after 5 attempts"; `Session::delete_for_me` now detects it, keeps the
  optimistic tombstone, and retries in the background (+15 s/+60 s/+2 m).
- **Revokes match AI replies.** AI replies were pushed with a synthetic `ai-…` id
  after their `sent` event, so a later REVOKE matched nothing; `Session::send_text`
  now returns the server `message_id` and `pushAiReply(chat, text, id)` binds the
  bubble to it (older cached replies keep their synthetic ids).
- **Audio messages / voice notes are playable.** `MessageView.audio {seconds, ptt,
  waveform}` from `audioMessage`; `media-ui.js` renders the official card (play/pause
  disc, blue seek line + drag-seek, waveform bars for `ptt`, countdown, `0:17 |
  6:46 am` footer, orange headphones chip). One at a time, progress painted via
  capture-phase `play/pause/timeupdate`. Locked by
  `audio_view_carries_duration_ptt_and_waveform` /
  `live_audio_messages_carry_the_player_metadata`.
- **Official channel implementation (verified).** The shipped package is a .NET 8
  WinUI shell hosting `web.whatsapp.com` in WebView2, so the channel RPC lives in
  the web layer. `WASmaxNewslettersGetNewsletterMessagesRPC` builds a **self IQ**
  (`<iq type=get xmlns=newsletter to="s.whatsapp.net"><messages type="jid"
  jid="<channel>@newsletter" count before|after/></iq>`; before/after are
  exhaustive); the reply carries `<plaintext>` protobufs with a `server_id`. The page
  cap is AB prop `channels_max_messages_batch_pull` (default **100**), which
  `fetch_channel_history` now clamps to.
- **Channel posts show the server's counters** (`<views_count>`,
  `<forwards_count>`, `<reactions>`) decoded into `MessageView.stats`
  (`ChannelStats`) as reaction pills + a share count, in the desktop app's 455 px
  column.
- The `hidden` attribute was overridden by author `display` rules, so channels still
  showed call buttons and the per-chat menu; `styles.css` now has
  `[hidden] { display: none !important; }`.
- **Channel posts are unwrapped before decoding.** Newsletter history wraps posts
  (`editedMessage`, `documentWithCaptionMessage`, view-once), so `imageMessage`
  captions decoded as empty text and were dropped; `decode_channel_page` now reads
  `message.get_base_message()` (verified with the `probe_channel_page` dump command).
- **Why "other channels" never synced.** The `<plaintext>` body can be a fanout
  envelope whose first length-delimited field holds the real message
  (`fields: [101]` media / `[106]` text); `decode_newsletter_body()` peels up to four
  layers, taking the channel from 4 stale posts to **26** (13 images, 2 videos, 11 text).
  Locked by `channel_fanout_envelope_is_peeled`.
- **Channel counters are capped** before they reach the UI: `ChannelStats` keeps the
  busiest three plus `reaction_total` (the server sends every emoji and tally).
- Channel videos fill their frame: the channel rule sets `max-height: none`
  (`.media-video`'s 380 px cap letterboxed portrait clips).
- **Reaction pills sit where WhatsApp Web draws them**: absolutely positioned at the
  bubble's bottom-left (`left: 6px; bottom: -11px`, outgoing on the right),
  `.has-reactions` adds `padding-bottom: 20px` + `margin-bottom: 10px` so the bubble
  doesn't grow; the pill is the solid `--panel-2` chip with a soft shadow.
- **Unsaved group members are no longer "Unknown"**: `senderHtml`/`previewOf` show
  the push name with Web's `~` marker plus the formatted phone (phone alone as a
  last resort), and history messages borrow a push name from another message of the
  same LID. Verified: 0 of 217 sender lines read "Unknown" (was 7).
- **Forwarding matches the desktop client.** `MessageView.forwarded`
  (`contextInfo.isForwarded` or `forwardingScore > 0`) draws the grey italic
  `↱ Forwarded` caption above the bubble; long-press → Forward opens the new-chat
  picker (groups included) and `forward_message(chats, id, text?)` relays the
  original protobuf with the quote chain stripped. Every message body is now cached
  so forwarding works (text fallback for older ones).
- **The message menu follows WhatsApp Web's layout**: icon rows for Reply, Copy,
  Forward, Edit (own text only), Message <sender> (groups), Delete (which opens the
  three-choice card directly). Reply quotes the original via
  `Session::send_text_reply` building `contextInfo { stanza_id, participant,
  remote_jid, quoted_message }` (text-only fallback when the body isn't cached).
- **A reply's quote links to its message**: `.quote[data-quote-id]` scrolls the
  original into view and flashes it (`.bubble.quote-target`), with a toast when it
  isn't loaded. Menu contents follow the direction (own: Reply/Copy/Forward/Edit/
  Message info/Delete; other in a group: Reply/Reply privately/Copy/Forward/Message
  <sender>/Delete). Ask Meta AI / Select / Report are omitted — no backend.
- **Select mode + honest disabled rows**: Select enters a checkbox mode with
  `.bubble.selected`, a floating `#select-bar` ("N selected", Forward/Delete);
  Ask Meta AI and Report are listed disabled with a tooltip and a toast.
- **Mentions are clickable everywhere** (including forwarded `@Push Name` text):
  `bodyHtml(text, mentions)` colours known names green (`rgb(6, 207, 156)`), tags
  `data-mention-jid`, and clicking opens the conversation (reusing the existing chat
  when the mention has no device suffix).
- **Delete is usable from the context menu.** The menu is capped at
  `max-height: calc(100vh - 16px)` with its own scrollbar and re-clamped by
  `clampMessageMenu()`; `confirmDelete()` marks the bubble immediately and fires the
  backend call in the background (`delete_for_me` = `DeleteMessageForMeAction`
  app-state mutation, `delete_for_everyone` = a protocol REVOKE), from a dedicated
  thread because the library's ack wait blocks its executor. "Delete for everyone" is
  hidden in the "Message yourself" chat.
- **Channels: avatars work.** Channel images ship as CDN *paths* (`/v/t61…`), which
  the old `http`-prefix check dropped; they're now prefixed with
  `https://pps.whatsapp.net`. The panel gained the media glyph and a **Discover
  channels** pane, and posts render as broadcast cards.
- **Channel history works through a protocol-level fetch.** REA inspection of the
  shipped package (`WhatsApp.Core.dll` managed PE, `WhatsApp.Networking.dll` =
  SMAX RPC layer) shows the client is .NET/WinUI over SMAX; the newsletter request is
  a **self IQ** addressed to `s.whatsapp.net` (addressing the channel JID is
  silently ignored). `fetch_channel_history` sends it with
  `client.send_iq_node(…, 6 s)`, decodes `<plaintext>` via `decode_channel_page`,
  caches media with `remember_message`, and returns the smallest `server_id` as the
  `before` cursor (verified: the channel 0 → 2 posts in **214 ms**).
- **Our own sends from the phone are no longer "new news"**: they don't bump the
  unread badge or notify, and "Message yourself" is forced to zero unread.
- **Chat-list previews resolve mentions** instead of leaking the raw JID (`hari
  @123456789012345 …` → `… @Alice …`), and stopped double-escaping (`&`).
- **Status playback is slower**: text 10 s (+up to 4 s for bodies over 80 chars,
  40 ms each) and images 8 s (were 7 s/5 s); videos play to the end. The image
  countdown starts with the frame, not on `<img>` load.
- **Status rings are one `box-shadow` pair** on the avatar (`0 0 0 2px var(--panel),
  0 0 0 4px accent`) instead of three nested rounded layers (which antialiased
  unevenly); concentric by construction. `viewed` swaps `--status-ring-colour`.
- **Status viewer prev/next** match WhatsApp desktop: 48 px circular buttons,
  translucent dark blur + shadow, thin 2.2 px white SVG chevrons, 24 px from each
  edge, and each arrow hides when there is nowhere to go (`updateStatusNav()`).
- **Status viewer follows the official layout**: progress bars + header in a centred
  640 px column over the full-bleed story colour, with the top shade painted by
  `#status-viewer::before` across the whole window. The **Viewed by** card matches
  the desktop dialog (500 px, 48 px avatars, `Today at HH:MM`); viewer rows for the
  broadcast chat or own devices are filtered out.
- **Status viewing controls**: text stories last 7 s (superseded by the 10 s entry
  above) and the viewer has Web's pause/play next to mute (also `Space`); pausing
  freezes the bar, the timer and videos exactly, and resuming continues from the
  remaining time.
- **Own-status id sync**: the UI pushes the ids of stories it owns to the session on
  startup and after every post (`watch_status_ids`), so view receipts match even for
  a story posted before tracking; receipts from our own devices are ignored.
- **The status text composer's emoji button opens Web's full emoji panel** (nine
  tabs, `Search emoji`, 12-per-row grid, insert at the caret without closing),
  rebuilt from `window.WA_EMOJI` so the client stays offline.
- **Who viewed my status**: the viewer shows the `👁 N` counter (plain white) and
  opens Web's **Viewed by N** dialog (500 px, 44 px avatars, `Today at HH:MM`,
  newest first). Data from `<receipt type="view">` for our own story ids →
  `UiEvent::StatusViewed`. Locked by `status_view_receipt_emits_status_viewed`.
- **Text statuses carry colour and font on the wire.** `post_status_text` takes the
  `0xAARRGGBB` background and the `Aa` font (mapped to `SYSTEM`/`SYSTEM_TEXT`/
  `FB_SCRIPT`/`SYSTEM_BOLD`); `StatusUpdate.background` (`#rrggbb`) paints received
  stories as authored. Posting returns the server id (so delete revokes it) and the
  recipient list is the union of known person chats and the saved address book.
  Locked by `status_background_is_hex_from_argb`.
- **Tapping My status opens Web's two-item menu** (Photos & videos / Text) instead
  of jumping into an editor; *Text* opens the full-screen composer (palette
  background, centred field, emoji/`Aa`/palette tools, privacy pill, circular send
  disabled until typed). The palette button cycles the sixteen Android status
  colours on every tap.
- **My status matches Web desktop**: no green "new update" ring on your own avatar,
  and the add affordance is the small green dot with a centred SVG plus.
- **Group avatars match Web**: a group without a photo draws the two-person glyph
  instead of initials, and a photo is requested once the webview knows it is
  connected (the restored-cache render ran before `get_status`). A failed photo
  (expired `pps` token) is evicted, falls back to the glyph, and retries after
  2 minutes; only `http(s)` URLs are cached.
- **Performance/memory pass on the webview** (behaviour unchanged): date labels
  memoized (`timeOf`/`stampOf`/`dayOf`, bounded maps dropped on day change) —
  192-row sidebar **10.2 ms → 3.4 ms**, 60-message chat **11.9 ms → 7.0 ms**;
  document/thumbnail hydration coalesced via `scheduleChatRender()`; profile photos
  patch one avatar (`patchChatAvatar`); chat search debounced 90 ms and capped at
  40 rows/section; no-op saves skipped by a state fingerprint (17.2 ms → 0.4 ms);
  scroll helpers reuse the metrics they already read.
- **Chat search follows Web**: results grouped into Chats / Contacts / Groups in
  common, the query painted in accent green, contacts show their phone; message
  bodies no longer produce rows; empty results say `No chats match "…"`.
- **Documents render as a Web-style file card** (extension badge, name, `pages ·
  TYPE · size`, first-page thumbnail, caption + timestamp footer): **View** (in-app
  preview) + **Save as…** for previewable files, single **Download** otherwise, with
  **Open** / **Show in folder** in the viewer. Older chats hydrate via the
  `document_info` command; quoted documents show `file.pdf (3 pages)`.
- **Media tiles**: downloads show the embedded thumbnail immediately and a stalled
  request becomes "Photo unavailable · Tap to retry" after 12 s. Expired media
  triggers the reupload handshake, now addressed to the **normalized** own JID
  (without the `:55` suffix) and fetched with the returned `direct_path`'s signature
  merged by `&`. Proven limit (`probe_media`): the server re-encrypts without
  sending a key (`enc_sha_matches: false`, a different rendition each time) and
  refuses reuploads while the path is still valid, so only a fresh send recovers old
  media; fresh media is unaffected.
- **Rust maintainability**: `ai.rs` (2,367 lines) became the `ai/` module tree, and
  `main.rs` (1,163 lines) became `main.rs` + `app.rs` + `state.rs` +
  `commands/{session,media,calls,history,status,contacts,groups,messages,links}`. The
  webview permission handler and window/tray setup now share one `.setup()` (the old
  duplicate setup overwrote the mic/camera hook).
- **Frontend maintainability**: `ui/main.js` split from ~6,900 lines into 21
  classic-script modules loaded in order; `main.js` is a 55-line bootstrap.
- **Chat scrolling**: switching chats lands on the newest message even after images
  load; reading older messages anchors the top visible bubble; new messages show a
  green dot plus a scroll-to-latest button that hides at the bottom.
- **Build speed**: `wa-core` split into feature modules and the serializable UI
  vocabulary moved to the `wa-types` crate; release uses `codegen-units = 16`,
  `lto = false` (was single-unit thin LTO).
- **Delete for everyone actually revokes.** A bare `ProtocolMessage`/REVOKE is
  ignored unless it carries the send pipeline's `edit: SenderRevoke` tag;
  `Session::delete_for_everyone` now calls `client.revoke_message(…,
  RevokeType::Sender)` and the session mutex is only briefly held. Verified end to
  end (the official client showed "You deleted this message").
- Linked-session history: on-demand `HistorySyncOnDemand` anchored on the oldest
  local message, older paging on scroll, 300 messages persisted per chat, plus a
  chat-export import fallback.
- Channel panels with real newsletter names and read-only post views.
- **Channel link embeds match the desktop client**: the preview card renders
  **above** the body (`body = preview + caption`) and paints the large
  `thumbnailDirectPath` image. `link_preview()` surfaces CDN paths as
  `LinkPreviewView.thumb_path`, `Session::link_thumbnail` fetches through the media
  connection's auth token and caches `lthumb-<id>` (LRU `link:<id>` + in-flight
  dedupe), and both event paths plus `fetch_channel_history` cache link-post bodies.
  `bodyHtml` shields URLs behind `\u0000i\u0000` placeholders before the `_text_`/
  `*text*` passes (so `utm_source=…` isn't split), trims trailing `&quot;`, and
  restores one anchor per URL without a second `linkify()` pass.
- **Channel image posts without a media key fetch again**: some payloads ship only
  `image.directPath` (no `url`/`mediaKey`/`fileEncSha256`); `util::plain_direct_media`
  + `fetch_media` build `https://<media-host><path>?auth=<conn.auth>` and save the
  plain object (verified: 0 retry tiles, native 1080×1350 images).
- **Every external anchor goes through one capture-phase handler** (`ui/wiring.js`):
  http(s) → `open_link`; channel post images → the post's own URL
  (`WAChatUtils.firstUrl`); internal `asset://…` (and WebView2's
  `http://asset.localhost/…`) are `preventDefault`ed in place so the viewer keeps
  working while the app never navigates.
- **The media viewer carries the official affordances**: crisp stroked SVG chevrons
  on a blurred disc, and a header with **Reply** (pre-fills the composer banner),
  **React** (six-emoji pill on the `applyReaction` path) and **Forward** (chat
  picker). The caption rides under the media in `#media-viewer-cap` with
  `bodyHtml` mention colouring.
- **Viewer arrows** are absolutely positioned (24 px from each window edge,
  vertically centred, 36 px discs, thin 17 px/2 px chevron) so they never move with
  layout or hover.
- **The filmstrip's scrollbar is hidden** (`scrollbar-width: none` +
  `::-webkit-scrollbar`), with a `wheel` handler forwarding `deltaY` to `scrollLeft`.
- WhatsApp-style mentions: LID/PN mentions resolve to contact or own display names.
- QR re-pairing: logout removes the local device store and opens a clean pairing
  session; the login screen clears its stale error when a new QR arrives.
- Sticker picker redesign plus the favorite-sticker send fix: optimistic bubble,
  immediate picker close, real server id, and no repeat sends while an upload is in
  flight.
- Own rail avatar resolves from the paired PN/LID and refreshes when the self chat or
  photo arrives.
- Connection badge reconciles with `get_status` while the webview believes it is
  offline.
- Status/stories: recent/viewed lists, `status@broadcast` history retrieval,
  progress-bar viewer, reply/reactions, text/image/video posting, and delete-own.
