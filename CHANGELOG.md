# Changelog

Newest first. Each entry keeps the mechanism (files, protocol constants, test
names) and the verification signal. Durable design notes, the feature inventory
and the roadmap live in [`AGENTS.md`](AGENTS.md).

## [0.2.0-rc.74] — 2026-11-04

- **Starred messages** (JS-only before): the bubble menu stars/unstars through
  `star_message`, `StarMessage` events fold into `Model::starred`, and Chats → kebab →
  **Starred messages** lists them across chats (sender, time, preview); picking one opens the
  chat.
- **Create group and add contact**, both from the Chats kebab: a group-name field with the
  address-book checklist (`create_group`), and a name + international number dialog
  (`save_contact`), which the app then opens as a chat.
- These close the remaining "list/menu" gaps from the JS→Rust move; still open: audio/video
  playback, export import, group member management, notifications/tray, AI loop and calls.



- **Chat actions are back** (they were JS-only after the migration): the chat header kebab now
  opens a card with Group info, Mute for 8 hours / 1 week / Unmute, Pin / Unpin, Mark as unread,
  Archive, Clear and Delete, all through the session's `chat_actions` (`pin_chat`,
  `mute_chat`, `archive_chat`, `mark_chat_unread`, `clear_chat`, `delete_chat`).
- **New-chat picker**: the green FAB asks the session for the address book
  (`Command::Contacts`), lists it in a searchable window and opens the chosen chat with its
  history request - the native replacement for the JS contact picker.
- README updated to the current state (official chat list, groups/communities, read sync,
  attachments, documents, installer, pure-Rust tree).



- **Read badges stay in step with the phone.** A `read`/`played` receipt or a
  `ChatReadMark` from the chat app-state clears the chat's unread count (already), and a
  `ChatReadMark { read: false }` now restores at least one badge, so marking a chat unread on
  the phone shows here too. The window polls the chat app-state every 45 s in addition to the
  live patches, because a missed `markChatAsRead` mutation would otherwise leave a stale badge
  forever - the same reason the official client re-syncs its state on resume.
- Verified on the real account that the sync path runs (`chat state sync`, participating
  groups) and that the rail/list badges derive from the same counters.



- **Groups are listed properly.** My rc.70 claim that groups need a relink was wrong: the
  protocol client exposes `groups().get_participating()`, and `Session::participating_groups()`
  now uses it. The window asks at start-up (and from the kebab → **Sync groups**), seeds each
  `@g.us` row with its subject and community flag, and requests their history - verified against
  the real account: **10 participating groups** appeared in the list with names, avatars,
  previews and unread counts.
- **Duplicate LID rows fold.** `UiEvent::ChatAlias` now merges the LID chat into its phone-number
  chat (name, unread, messages deduplicated by id), so a group/contact stored twice appears once.
- The kebab menu gained **Sync groups**; the relink action remains for importing older message
  history, not for the group list.



- **Groups in the chat list.** The session now runs the chat app-state collection (pins,
  archives, mutes) at start-up (`Session::sync_chat_state`), and every chat it mentions is added
  to the sidebar - group metadata then fills in the subject and the community flag, and the
  Groups/Communities pills count them. Chats learned from live traffic/receipts are added the
  same way. `Session::known_chats()` exposes the JIDs the session has seen.
- **The Chats kebab is a real menu** (rounded card, icon rows): Settings, Sync chat history,
  Refresh statuses and Log out - previously it jumped straight to Settings. `WA_DESK_MENU=1`
  opens it for screenshots.
- **Visual parity pass against a real capture of the official client** (Windows GDI capture via
  `cargo run -p wa-desktop-native --example capture_window -- WhatsApp shot.png`): search icon
  inside the rounded field, 72 px rows with 48 px avatars, outlined filter pills, the green FAB,
  and a Communities entry on the rail that jumps to the Communities filter.
- **Relink to import everything.** WhatsApp only sends groups and older chats to a *newly
  linked* device; there is no protocol call to enumerate existing groups. Settings → Chat
  history now has **Unlink and pair again**, which returns to the QR gate so the next link
  imports the complete list.
- Rows with no history sort by name after the timestamped ones, so the list stays stable while
  the batches land.



- **Chat list order matches the phone.** Rows sort by their last message time, newest first, and
  ties fall back to the name so contacts without history are stable instead of appearing in
  arbitrary JID order. The background backfill now resumes on **every** connection (six
  batches, eight chats each) instead of only when the cache is empty, so rows that started
  without a timestamp move up as their history arrives.
- Settings → Chat history states the limitation plainly: WhatsApp only pushes the complete chat
  list to a newly linked device, so chats older than the backfill still need a relink.



- **The chat list layout now matches the official client**: a 58 px navigation rail with unread
  and unseen-status badges and your own photo at the bottom, a "Chats" header with the kebab and
  the green new-chat FAB, the rounded search field, and the **All / Groups N / Communities N /
  Unread N** filter pills (they filter the list and the counts are live). Selecting no chat shows
  the round empty-state actions instead of a bare label.
- **Your own identity**: `UiEvent::SelfIdentity` carries `own_jid` + push name at session start,
  so the sidebar avatar loads your profile photo (it was blank because `me_jid` was never set)
  and the self chat shows your name.
- **History sync runs in batches**, like the phone: after the address-book pass the app keeps
  requesting the next eight chats every eight seconds (six rounds), and Settings → Chat history
  has "Sync more chats". Groups and communities appear as their history arrives.
- **Fixed: preview runs wrote the demo chats into the real cache.** `--preview` no longer saves
  `chats.json` (update or exit), so the sample "Team/Ada/Budi" rows cannot leak into a real
  account.
- Emoji fall back to the OS font (Segoe UI Emoji / Noto), so 🙏 👍 🎉 render as glyphs.
- The last two vendored Python CI scripts were deleted: **the repository contains no code in any
  language other than Rust** (Cargo manifests, docs and CI YAML aside).
- 35 native tests, clippy `-D warnings` clean, selftest green.



- **Fixed: the chat list was empty after a restart.** WhatsApp only replays history to a newly
  linked device, and the native app never kept a local chat list, so a linked account opened to
  an empty sidebar. The model now mirrors the list (names, unread, newest 80 bubbles per chat) to
  `chats.json` beside the session, loads it at startup, and saves it on a 2 s debounce and on
  exit. Tests: `the_chat_cache_round_trips`, `contacts_seed_the_chat_list_before_history_arrives`.
- **Recovering history on connect.** When there is nothing cached, `Command::InitialSync` seeds
  the sidebar from the linked session's address book and asks the phone for a history batch
  (eight chats, 3 s apart, the same pacing as the old client). Settings → Chat history exposes
  the same request manually, next to a "Refresh statuses" action.
- **Profile photos.** `Command::Avatar` resolves a contact's photo through
  `session.contact_photo`, downloads it with `ureq` on a worker thread, masks it into a circle
  and caches the texture; initials remain the fallback while it loads or when there is none.
  The sidebar rows, the header and your own avatar all use it.
- **Status history.** Opening Status asks the session for the recent `status@broadcast` window
  (`fetch_status_history`), so stories posted while the app was closed appear.
- **No console window.** Release builds set `windows_subsystem = "windows"`; debug builds keep
  the console so `--selftest` and the updater checks stay readable.
- **Contacts are phone numbers, not JIDs.** `session.contacts()` is keyed by bare numbers; the
  initial sync now appends `@s.whatsapp.net` before seeding the sidebar or requesting history,
  which is why avatar lookups said "unknown server" and history requests were dropped.
- **History requests on open.** Tapping a chat with no cached messages sends
  `Command::ChatHistory` for that chat (`sync_chat_history`, 30 messages); WhatsApp only replays
  history on demand for an existing device.
- **Attach files.** The composer has a paperclip next to emoji and stickers: it opens the native
  file dialog (`rfd`), sends the file through `send_attachment` with a MIME type from its
  extension and the composer text as the caption (48 MB limit, checked before reading). A
  short note under the composer reports "Sending …" and the result.
- **Ctrl+V attaches a copied file.** On Windows `clipboard.rs` reads `CF_HDROP` from the OS
  clipboard, so copying a file in Explorer and pressing Ctrl+V in the chat sends it; text paste
  is untouched.



- **Documents render as the official card.** A `document` bubble no longer falls back to plain
  text: it draws the Material `description` icon on a rounded tile, the file name (wrapped), the
  size and page count (`1.8 MB · 12 pages`), the caption underneath and the clock in the corner,
  with the same tail and colours as text bubbles. `Model::file_size` formats B / kB / MB / GB
  the way WhatsApp does, and `--preview` now includes a PDF so the card is visible in
  screenshots.
- Test: `file_sizes_read_like_whatsapp` (812 B, 4.2 kB, 1.8 MB, 2.5 GB); the raster test still
  guards the Material icons.



- **The window now looks like WhatsApp, not like default egui.** The theme was stored but never
  applied; `theme.rs` now installs the official `--WDS-*` palette (dark surface `#161717`, rail
  `#1D1F1F`, elevated `#242626`, outgoing `#144D37`, accent `#21C063`; light panel `#FFFFFF`,
  rail `#F7F5F3`, wallpaper `#F5F1EB`, outgoing `#D9FDD3`) together with WhatsApp's type scale,
  rounded popups and shadows. Light / Dark / System actually switches now.
  - **Sidebar**: own avatar, "Chats" header with vector icons, rounded search field, and rows
    with a 44px initials avatar, name, preview, clock and the green unread pill; the open chat
    gets the selected-row tint.
  - **Conversation**: a header with the chat avatar and presence line, the wallpaper behind the
    messages, bubbles measured from a `LayoutJob` so they hug the text with WhatsApp's tail
    (one square corner), sender names coloured per sender in groups, the clock inside the
    bubble, and a short chat anchored to the composer.
  - **Composer**: rounded pill input with emoji / sticker / send buttons.
  - **Message menu**: the rounded card with the six quick reactions on top, then Reply / Copy /
    Forward / Edit / Delete rows with drawn icons and a hover highlight - the same shape as the
    removed JS menu, in Rust.
- **Icons are the real Material Symbols.** The set in `wa_ui::icons_data` (search, more_vert,
  donut_large, campaign, settings, mood, sticker, send, reply, shortcut, content_copy, edit,
  delete) is rasterised once per size and colour with `resvg` and cached on the egui context
  (`icons.rs`); a test asserts the icons paint visible pixels. Nothing is drawn from font
  glyphs, so they render identically on every machine.
- **`--preview` and `--screenshot <path>`**: a session-free sample chat renders the window and
  saves one settled frame with `ViewportCommand::Screenshot`, so the UI is verified visually
  without a linked account (the screenshots used for this release came from it).
- 28 native tests (theme resolution and the official token values are asserted); clippy
  `-D warnings` clean.



- **CI: the serial `check` job is now two parallel jobs.** `lint` (fmt, clippy, `cargo audit`) and
  `test` (`cargo test --workspace`, native selftest) ran back to back and were the critical path
  at ~9 minutes. They now run side by side on separate runners with their own caches, so the job
  that gates the release is the slower of the two rather than the sum.
- **The installer asset is `WA-Desk-Setup.exe`.** GitHub replaced the spaces in
  `WA Desk Setup.exe` with dots on upload; the hyphenated name survives `gh release create` and
  is safe in URLs and scripts.



- **The Windows installer is back.** The MSI disappeared with the Tauri bundler in rc.57; there
  is a real installer again, written in Rust: `apps/windows-installer` holds a setup stub, and
  `pack-setup` glues the app binary to it behind a 16-byte trailer (`WADSETUP` + length). Running
  `WA-Desk-Setup.exe` installs per-user into `%LOCALAPPDATA%\Programs\WA Desk` (no admin, no
  MSI runtime), writes `Uninstall WA Desk.cmd`, creates Start Menu and Desktop shortcuts through
  `IShellLink` COM, and starts the app. The release job now uploads it next to the raw binaries.
- Verified locally: packing rc.63 produced a 45,510,672-byte setup, running it installed
  `WA Desk.exe` at exactly the app size, created both shortcuts and launched the app; the
  payload/pack functions have four unit tests (round-trip, plain binary, truncated trailer,
  per-user install dir).
- **CI is much faster.** The dependency cache was keyed on `Cargo.lock`, so every version bump
  threw away the compiled tree and each tag rebuilt the whole crypto stack. The workflow now
  keys the cache on the Rust version instead (with `restore-keys` fallback), sets
  `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0`, installs `cargo-audit` from a prebuilt
  binary instead of `cargo install`, runs `apt-get` with `--no-install-recommends`, adds a
  `concurrency` group that cancels superseded runs, and caps each job with `timeout-minutes`.
- The release job now needs `[check, build]`, so a red test run cannot publish a tag.



- **Fixed the red `check` job on v0.2.0-rc.61.** The updater's test fixtures hardcoded the
  Windows asset name (`wa-desk-native.exe`), so on the Linux runner `parse_releases` found no
  matching asset and the tests panicked. The fixtures now build their JSON from `asset_name()`,
  including a newer release that only ships the *other* platform's binary (it must be skipped).
  No runtime change: the rc.61 binaries are correct.



- **Fixed: the updater could not find or download a release.** Two things were wrong. The
  native app had no updater at all, and the obvious GitHub endpoint cannot work here: **every
  WA Desk release is a pre-release**, so `/releases/latest` answers 404. The new
  `apps/desktop-native/src/updates.rs` lists `/releases?per_page=20` instead, skips drafts,
  keeps the highest tag newer than `CARGO_PKG_VERSION` that actually ships this platform's
  asset (`wa-desk-native.exe` / `wa-desk-native`), and downloads it with `ureq`. Version order
  is the numeric core first - a bigger core always updates (`0.2.0-rc.60` → `0.3.0-rc.1`, →
  `1.0.0`) - then a final release above any pre-release of the same core, then the `rc.N`
  number compared numerically (`rc.10` beats `rc.9`). Tests cover all of those cases.
  - Installing renames the running exe to `.old`, moves the download into place, spawns it
    with `--restarted` and exits; the new process waits 1.5 s so the old one releases the
    database and the single-instance mutex.
  - The check runs once after pairing and on demand from **Settings → Updates**, which shows
    the installed version, the available tag and size, a notes link, download progress and
    the restart button; the sidebar shows an `⬆ Update` shortcut when one is waiting.
- Verified against the live repository: `--check-updates` reports `up to date (0.2.0-rc.61)`,
  `WA_DESK_UPDATE_FROM=0.2.0-rc.59 --check-updates` reports
  `v0.2.0-rc.60 (45161472 bytes) …/wa-desk-native.exe`, and `--download-update` downloaded
  `45161472 bytes`, matching the asset size exactly.
- 24 native tests (four new: pre-release ordering, newest-release-with-asset selection,
  up-to-date/no-asset handling, asset naming); clippy `-D warnings` clean.



- **Message actions in the native window.** Right-clicking a bubble opens WhatsApp's menu:
  the six quick reactions, Reply, Copy, Forward, Edit (only your own text), Delete (only for
  gone messages is Delete kept out). The rows come from `wa_ui::menus::message_menu`, so the
  native window applies the same rules as the old shell.
  - **Reply** shows a preview card above the composer (`wa_ui::actions::reply_preview`) and
    sends through `send_text_reply` with the quoted body.
  - **Edit** loads your text into the composer, relabels the button `Edit`, and sends
    `edit_message`; cancelling drops the draft. `MessageEdited` events rewrite the bubble.
  - **Delete** asks "Delete message?" via `wa_ui::menus::delete_confirm`, offering
    *for everyone* only when the message is yours and the chat is not "Message yourself";
    *for me* removes it locally as well. `MessageDeleted` events follow the same path.
  - **Forward** opens a searchable, multi-select chat picker and sends `forward_message`
    once for every chosen chat.
  - **React** sends `send_reaction` with the sender's participant id, shows our pill
    immediately and folds `Reaction` events (`reactions_for`), ours in blue.
- **The last non-Rust files are gone.** All 23 `.mjs`/`.ps1` dev scripts (CDP harnesses, theme
  audit, window capture, icon/emoji generators, the WebView dev server) and `package.json`
  were deleted; the icon generator is now the Rust example
  `cargo run -p wa-ui --example gen_icons`, which reproduces `icons_data.rs` byte-for-byte
  from `scripts/material-paths.json`. The repository now contains **0** `.js`, `.mjs`, `.ps1`
  and `.bat` files.
- **CI runs on release tags only.** `build.yml` no longer triggers on pushes to `main` or on
  pull requests - only `push: tags: ["v*"]` and a manual `workflow_dispatch` - so a normal
  commit does not spend a runner. The `check` job (fmt, clippy, tests, native `--selftest`)
  still gates every release tag, and the `release` job still publishes the binaries.
- 20 native tests (four new: reply/edit composer state, delete ownership + removal, forward
  picker/edits/revokes, reaction pill replacement); clippy `-D warnings` clean;
  selftest prints `{"delete":"Delete message?","delete_everyone":true,...,"forward":1,
  "reactions":2,...}`.



- **In-chat search in the native window.** The conversation header's `Search` button opens a
  bar with the `2/5` counter, `▲`/`▼` stepping and `✕`/`Esc` to close. Matching comes from
  `wa_ui::search` (`matched_indices`, `count_label`, `hit_index`); the window paints every
  occurrence on a yellow band inside the bubble and outlines the hit, then scrolls it into
  view. The search resets when another chat opens.
- **Emoji and sticker pickers.** The composer now carries `Emoji` and `Sticker` buttons.
  The emoji panel searches the same 2,300-glyph catalogue the old shell used, keeps recents
  and inserts through `wa_ui::emoji::insert_text`. The sticker panel lists the account's
  favourites (fetched with `fetch_favorite_stickers`, sent with `send_favorite_sticker`)
  above the stickers already cached from chats (sent with `forward_sticker`). Sticker
  bubbles now paint their image without a bubble, 128px wide.
- **Media viewer.** Clicking a photo or video opens a full-window stage with a filmstrip,
  `◀`/`▶`, arrow keys and Esc; `Save` copies the cached file into `Downloads\WA Desk`
  in Rust. Photos and stickers are downloaded on demand (one request per message).
- 16 native tests (four new: search stepping/wrapping, emoji insert + recents, viewer paging,
  sticker collection); clippy `-D warnings` clean across the workspace;
  selftest prints `{"channels":1,"chats":3,"emoji":160,...,"search":"1/1",...,"stickers":0,
  ...,"viewer_items":1}`.

## [0.2.0-rc.58] — 2026-11-04

- **Documentation now matches the code: no Tauri, no WebView, no unused libraries.**
  The README states what was removed in rc.57 — the Tauri shell, `wry`,
  `webview2-com`, `webkit2gtk`, the `tauri-plugin-*` crates, MSI/NSIS bundling and
  the 13-file JavaScript front end — and lists the only additions the native app
  needs (`eframe`, `egui`, `resvg` for QR, `image` for photo decoding).
  `SECURITY.md` drops the WebView permission/updater rows, `AGENTS.md` drops the
  Tauri dev-mode and tray/updater notes, and the GitHub repository description and
  topics describe the native app.
- Verified after the cleanup: `Cargo.lock` contains **0** `tauri`, `wry`,
  `webview2` and `webkit2gtk` entries (4 native UI crates present), no crate
  manifest mentions tauri, and the repository still holds **0** `.js` and **0** UI
  `.html` files.

## [0.2.0-rc.57] — 2026-11-04

- **The WebView front end is gone: the app is now Rust all the way down.**
  `apps/desktop` (the Tauri shell and its 13 JS files) and the CDP e2e suite are
  deleted, `scripts/build-emoji.js` and the five JS audit scripts went with them,
  and the workspace builds only `wa-types`, `wa-core`, `wa-ui` and
  `apps/desktop-native`. **The repository now contains zero JavaScript files and
  zero HTML UI files**; `wa-desk-native` (`eframe`/`egui`) is the shipped app and
  `dist\WA Desk.exe` is built from it.
  - CI now checks `cargo test --workspace`, runs the native `--selftest`
    headlessly, and the release job publishes the native binaries instead of
    MSI/NSIS installers.
- **Fixed: reading on the phone left the unread badge behind.** A `read` (or
  `played`) receipt arriving for a chat now clears that chat's unread counter and
  a `delivered` receipt does not — previously only opening the chat locally or a
  `ChatReadMark` cleared it, so a chat read on the phone kept its badge until it
  was opened here. Regression test covers both paths.
- **Group info in the native window.** `Command::GroupInfo` loads the metadata
  through the session; the screen shows the subject, description, the
  `Group - N members` subtitle from `wa_ui::labels::group_member_count` and the
  member list with **admin** chips, admins first and names resolved from the chat
  store where known.
- 12 native tests; clippy clean across the workspace; selftest prints
  `{"channels":1,"chats":3,"group_members":2,...,"theme":"light"}`.

## [0.2.0-rc.56] — 2026-11-03

- **The native window has a Settings screen.** `settings.rs` stores the theme and
  the AI form as JSON; the **API key is never written to disk**, and every rule is
  `wa-ui`'s: `resolve_theme`, `theme_label` and `ai_save_plan` (so a blank key
  keeps the stored one and a non-http base URL or blank model is rejected with the
  same copy the WebView shows).
- `app.rs`: a **Settings** button opens Light / Dark / System choices (showing
  both the stored choice and what is actually painted today), plus the AI form —
  mode and `/ai` requirement, base URL, model, vision model, system prompt and a
  masked key field with a hint — and Save, which validates through `wa-ui` and
  reports the toast or the first error.
- 3 new native tests (theme resolution, AI validation, round-trip that asserts the
  key is absent from the file). `--selftest` covers the theme and AI rules too
  and reports `"theme":"light"`.
- 11 native tests, clippy clean across the workspace, window starts and stays up.

## [0.2.0-rc.55] — 2026-11-03

- **The native window lists followed channels.** `Model` folds `ChannelInfo`
  into `ChannelRow` (jid, name, subscribers) and keeps a channel's posts in the
  chat store, so `channel_posts()` hands them back oldest-first while
  `channel_rows()` orders channels by newest activity and labels each with
  `wa_ui::labels::format_followers`. Channels are excluded from
  `visible_chats()`, so they never show up as chats in the sidebar.
- `app.rs`: a **Channels** button beside Status opens a rail of followed
  channels (name + follower line, selected state) with the posts beside it,
  painted without a composer and captioned read-only.
- `--selftest` adds a channel with 84,300 subscribers and one post: it lists
  under Channels as `84.3k followers`, its post body comes back, and the
  channel is absent from the chat rows. Report gains `"channels":1`.
- 8 native tests, clippy clean across the workspace, window starts and stays up.

## [0.2.0-rc.54] — 2026-11-03

- **The native window shows status/stories.** `Model` folds `Status` and
  `StatusViewed` into per-author rows, and the panel/viewer lean on `wa-ui`:
  `status::viewer_groups` orders posters newest-first, `next_position` steps
  frames and groups, `frame_duration` + `progress_ratio` drive the progress
  segments and `viewer_chrome` decides the arrows and the own-story view count.
  A view receipt marks the frame seen and records the viewer.
- `app.rs`: a **Status** button next to the title switches the central panel to
  the poster rail — green ring for unseen, grey for seen, with update counts.
  Opening a poster shows the story viewer: progress segments (finished frames
  full, the active one filling), the frame's own background colour, its text or
  media placeholder, prev/next and close. A frame that runs out advances on its
  own, like the phone.
- `--selftest` asserts the whole flow: two posters newest-first and unseen,
  opening the newer one shows next-but-no-prev, progress within 0..1, the poster
  becomes seen, stepping lands on the other poster, closing clears the viewer.
  Report now includes `"status_groups":2`.
- 8 native tests, `wa-ui` 232, clippy clean across the workspace, window starts.

## [0.2.0-rc.53] — 2026-11-03

- **The native window shows media.** A photo bubble decodes the file `wa-core`
  caches (via the `image` crate) into an `egui` texture, scaled to 240 px with
  its caption underneath; `video`, `audio`, `document` and `sticker` fall back to
  a placeholder word. The conversation asks the session for each photo **once**
  (`Command::FetchMedia` → `Session::fetch_media`), and the result returns on a
  media channel into `Model::media` as `Loading` / `Path` / `Failed`.
- `wa_ui::chat::media_placeholder(kind)` is the shared word source ("Photo",
  "Video", "Document", "Sticker", "Media" fallback), so the two front ends use
  the same labels.
- `--selftest` also checks a photo: it labels itself `Photo`, the download is
  requested exactly once and the cached path lands in the model; the report now
  carries `"photo_body":"Photo"`.
- 8 native tests (the new one covers the media lifecycle), 232 `wa-ui` tests,
  clippy clean, window starts and stays up.

## [0.2.0-rc.52] — 2026-11-03

- **The native window links an account: QR, pairing code and status.**
  - `login.rs` is a pure gate state machine with 4 unit tests: a QR fills it, an
    expired code offers a restart, a pairing code is its own screen, `Paired`
    and `Connected` leave the gate, `LoggedOut` returns to it.
  - `app.rs` shows the gate instead of the chat UI until the account is linked:
    the QR that `wa-core` emits is rasterised with `resvg` (texture cached per
    code, so a rotating QR decodes once), the pairing code is shown large, a
    "Request a new code" action reconnects, and a phone-number field asks for a
    pairing code. Every action runs through the session on the tokio runtime.
  - `--selftest` now walks the gate as well and reports `"linked": true`.
- 7 native tests (4 for the gate, 3 for the model). `cargo clippy --workspace
  --all-targets -- -D warnings` is clean, and the window starts and stays up.

## [0.2.0-rc.51] — 2026-11-03

- **A native front end exists: `apps/desktop-native` (binary `wa-desk-native`).**
  It renders with `eframe`/`egui` — no WebView, no HTML, no JavaScript — and
  drives the same `wa-core` session the Tauri app uses. Every user-visible
  string (chat preview, clock stamp, title fallbacks) comes from `wa-ui`, so the
  two front ends cannot drift apart.
  - `model.rs`: the chat store, folded from `UiEvent`s — live messages, history
    replay, sent echoes, read marks, deleted chats, pairing/QR/connection status
    — plus the search filter, the unread rule (an open chat never gains a badge)
    and `open_chat` clearing it.
  - `app.rs`: sidebar (search, preview, stamp, unread pill) and the conversation
    (own/other bubble colours, sender + clock header) with a composer that sends
    through the session and reports typing.
  - Session commands (`SendText`, `MarkRead`, `Typing`) are executed on the
    tokio runtime that owns the session.
  - `--selftest` runs the whole model headlessly, applies a scripted event
    sequence and prints what the window would show:
    `{"chats":2,"first":"Team","preview":"ok","stamp":"9:40 am","status":"Connected"}`
    — it exits non-zero if any expectation fails, so the native UI is verifiable
    without a display.
- 3 unit tests for the model; `cargo clippy --workspace --all-targets -D warnings`
  is clean with the new crate, and the Tauri app still builds.

## [0.2.0-rc.50] — 2026-11-03

Bulk release (one cut for the whole batch).

- **`new-chat.js` and `settings.js` are folded into `chat-list.js` — 15 → 13 JS
  files**, with two more decisions in Rust:
  - `wa_ui::settings::settings_back(page)` — a subpage returns to the home list,
    home closes Settings (command `ui_settings_back`).
  - `wa_ui::picker::forward_toast(sent)` — `Message forwarded` for one, `N
    messages forwarded` otherwise (command `ui_forward_toast`).
- 2 new `wa-ui` unit tests (231 total, plus 3 contract). The picker rows,
  group dialog and every Settings page were already Rust; what remains in those
  files was DOM painting, which now lives in `chat-list.js`.
- Audits: `check-undefined` 20 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.49] — 2026-11-03

Bulk release (one cut for the whole batch).

- **`media-viewer.js` and `main.js` are gone — 17 → 15 JS files.**
  - `wa_ui::viewer::zoom_state(next)` owns the zoom rules: the 1..6 clamp,
    2-decimal rounding, the near-1 snap that stops the "zoomed" class flickering,
    and the in/out button states; `next_speed(current)` cycles 1 → 1.5 → 2 → 0.5
    and formats the label (`1.5x`). Commands `ui_viewer_zoom`,
    `ui_viewer_speed`; `setViewerZoom` and the speed button apply them.
  - The rest of the viewer (video transport, filmstrip, zoom/pan) moved into
    `media-ui.js`; the bootstrap (`wire()`, listeners, `get_status`) moved into
    `wiring.js`.
- 2 new `wa-ui` unit tests (229 total, plus 3 contract).
- e2e: the scroll check no longer asserts a stick flag that a repaint can race;
  it asserts what a reader sees (disc shown, unread dot, re-attach), and both
  script-count checks derive from `index.html`.
- Audits: `check-undefined` 20 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.48] — 2026-11-03

Batch release (one cut for the whole batch, not one per file).

- **`chat-menu.js` + `message-actions.js` are gone — merged into one `menus.js`
  whose open/close state is a Rust transition (19 → 17 JS files).**
  `wa_ui::menus::menu_session(current, event)` is the state machine: `open`
  replaces whatever popover was up, `toggle` on the same kind+target closes it,
  `close` always does, and an empty kind opens nothing. The chat list menu, the
  message menu, the sidebar kebab, the member menu and the status add menu all
  share it (`ui_menu_session`), so opening the message menu now closes the chat
  menu — previously both could be visible at once. The remaining DOM code for
  both files lives in `menus.js`, loaded before `wiring.js`.
- **Peer lookup targeting is one call.** `wa_ui::state::peer_targets` picks the
  chats that still need a `usync` lookup — LIDs always, phone-number chats only
  while the name is still the bare number, never groups, our own ids or chats
  already tried — capping the batch. `resolvePeers` used to make one
  `ui_placeholder_name` round trip per chat; it now makes exactly one call
  (`ui_peer_targets`).
- **The draft cap is Rust's.** `ui_draft_limits` returns
  `actions::MAX_DRAFT_FILES`, which `chats.js` mirrors at boot instead of
  hard-coding 10.
- 2 new `wa-ui` unit tests (227 total, plus 3 contract) and a menu-session CDP
  e2e section: the merged module is the only one loaded (17 scripts), the
  one-popover transition, the peer-target selection plus the draft cap, and a
  live pass where a right-click opens the chat menu and clicking a message closes
  it.
- Audits: `check-undefined` 20 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.47] — 2026-11-03

- **`emoji.js` is deleted — 19 → 18 JS files, with real Rust behind it.**
  `wa_ui::emoji` gained the picker's bookkeeping rather than just its markup:
  - `remember_recent(recents, char)` — moves the picked emoji to the front,
    dedupes the whole stored list (older builds could keep duplicates) and caps
    it at `RECENTS_CAP` (24).
  - `picker_tab(tab)` — which of the three views a tab shows, whether the
    sticker grid must reload, and which element takes focus.
  - `insert_text(value, start, end, insert)` — replaces the selection (reversed
    or out-of-range indices are clamped) and reports the new caret, so the
    composer no longer slices strings by hand.
  - `composer_has_text(text)` — the send button's "needs real text" gate.
  Commands `ui_emoji_remember`, `ui_picker_tab`, `ui_insert_text`,
  `ui_composer_has_text`; the remaining DOM code moved into `stickers.js`.
- 4 new `wa-ui` unit tests (225 total, plus 3 contract) and an emoji CDP e2e
  section: the file is no longer loaded, dedupe/cap, the three tabs, caret
  insertion and a live pass that types, inserts 😀 at the caret, disables send on
  an empty composer and flips tabs.
- The e2e's per-file "is it still loaded?" checks now compare against the number
  of `<script src>` tags in `index.html` instead of a hard-coded count, so they
  stop needing an edit every release.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.46] — 2026-11-03

- **`attachments.js` is deleted — 20 → 19 JS files.** Its last decisions became
  part of the Rust attachment plan and the DOM glue sits in `chats.js`:
  - `wa_ui::actions::attachment_send(size, mime, caption)` extends the check with
    the two things the `send_attachment` call needs: `send_mime` (a blank pick
    type becomes `application/octet-stream`, a real one is passed through
    untouched while the kind comparison stays lowercased) and `caption`
    (trimmed, `None` when blank). `attachment_check` delegates to it with an
    empty caption, so nothing else changed. Command `ui_attachment_send`.
  - `fileToBase64` and `sendFile` moved into `chats.js`; `sendFile` now sends
    `check.send_mime` / `check.caption` instead of re-deriving them.
- 1 new `wa-ui` unit test (221 total, plus 3 contract) and an attachments CDP e2e
  section: the file is no longer loaded (19 scripts), the send plan for a real
  image / a blank type / an oversized file, both helpers still present, and a
  live draft stack that stages a file and base64-encodes it.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.45] — 2026-11-03

- **`contacts.js` is deleted — 21 → 20 JS files.** Its remaining decisions became
  Rust and its DOM glue sits in `new-chat.js` (the picker file):
  - `wa_ui::picker::group_modal(mode)` — the New group / Add members dialog's
    title, submit label, whether the subject field shows, which field gets focus
    and the success toast (`Group created` / `Members added`). `openGroupModal`
    and `openAddMembers` are async now and paint `plan.*`.
  - `wa_ui::state::created_group_chat(jid, name, now)` — the row a newly created
    group gets (`isGroup`, empty message list, `last: null`, `lastTs: now`,
    `unread: 0`) instead of a hard-coded object literal.
  Commands `ui_group_modal`, `ui_created_group_chat`.
- 2 new `wa-ui` unit tests (220 total, plus 3 contract) and a group-dialog CDP
  e2e section: the file is no longer loaded (20 scripts), both plans, the created
  row, and a live pass that opens both dialogs and checks their chrome
  (`New group`/`Create` with 287 contacts, then `Add members`/`Add` with the
  subject hidden).
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.44] — 2026-11-03

- **`group-info.js` is deleted — 22 → 21 JS files.** Its remaining decision is
  Rust and its DOM glue now sits in `chat-list.js` beside the other group and
  community plumbing:
  - `wa_ui::chat_actions::leave_group_confirm(name)` — `Leave "Team"?`, and
    `Leave this group?` when the group has no name yet (the old template literal
    would have rendered `Leave ""?`). Command `ui_leave_group_confirm`.
  - `openGroupInfo`, `leaveCurrentGroup`, the member menu and
    `applyMemberAction` moved unchanged (they already applied the Rust plans
    `ui_group_members`, `ui_member_action`, `ui_member_menu_position`); the file
    and its `<script>` tag are gone.
- 1 new `wa-ui` unit test (218 total, plus 3 contract) and a group-info CDP e2e
  section: the file is no longer loaded (21 scripts), all four handlers still
  exist, the confirmation copy comes from Rust, and a live group screen opens
  with its member kebab.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.43] — 2026-11-03

- **`chat-search.js` is deleted — one fewer JS file (23 → 22).** Its hit-index
  bookkeeping is now Rust and its DOM painting lives in `chat-render.js`:
  - `wa_ui::search::hit_index(total, requested)` wraps a requested hit into
    range (arrows step past either end) and returns `-1` when there are no hits;
    `keep_hit(total, current)` keeps the position when the query re-runs over a
    list that may have shrunk. `scrollToChatSearchHit` and the keep-index path in
    `runChatSearch` are the only JS that changed; the TreeWalker/CSS-Highlight
    painting moved verbatim into `chat-render.js`.
  - `chat-search.js` and its `<script>` tag are gone, and `runChatSearch` now
    sets the box's value before running (the counter reads the input, and a
    repaint re-runs the search with it — the e2e caught my first version leaving
    it blank, which made the counter vanish after one step).
  Commands `ui_search_hit_index`, `ui_search_keep_hit`.
- 1 new `wa-ui` unit test (217 total, plus 3 contract) and an in-chat search CDP
  e2e section that asserts the file is not loaded, `runChatSearch` still exists,
  and a live search over 200 hits steps `1/200 → 2/200`, wraps back to `200/200`
  and clears its marks.
- e2e now picks a chat that contains **our own** message for the delete-dialog
  check ("Delete for everyone" is only offered for those), which the new section
  had shifted out of first place.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` **0**,
  `check-state-fields` 0/38.

## [0.2.0-rc.42] — 2026-11-03

- **The contact-plan views and the sent-echo fold moved to Rust.**
  - `wa_ui::state::contact_chat_views(chats, me_jid, me_lid)` builds what
    `ui_contact_plan` consumes: the own chat flagged with `is_self_jid` (user-part
    comparison, so a device suffix no longer hides it — the old code compared the
    raw strings) and each chat's sender list deduped/cleaned. `contactChatViews`
    is async and both call sites await it.
  - `wa_ui::incoming::sent_plan(entries, id, timestamp, is_self_chat)` folds the
    `sent` echo into the newest optimistic bubble: the server id, the event's
    timestamp (or the bubble's own when the event has none), the delivery state
    (`delivered` in "Message yourself"), and **`set_last`** — the chat's last row
    only moves when this is the newest message, so a delayed echo can no longer
    reorder the sidebar. It replaced `ui_newest_pending` + `ui_self_chat_status`,
    which are deleted along with the now-unused `newest_pending` helper.
  Commands `ui_contact_chat_views`, `ui_sent_plan` (171; `ui_newest_pending` and
  `ui_self_chat_status` removed).
- 2 new `wa-ui` unit tests (216 total, plus 3 contract) and a
  contact-view/sent-echo CDP e2e section: the own-chat flag including a suffixed
  jid, deduped senders, the newest vs delayed echo, the self-chat delivery state,
  and a live `addSent` that completes a staged pending bubble.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.41] — 2026-11-03

- **Star, mute and the contact-sync gate moved to Rust.**
  - `wa_ui::chat_actions::star_plan(id, starred_now, from_me, sender)` — the
    toggle direction (`starred: !now`), the participant rule (only someone
    else's message carries an author) and the toast; a message without an id is
    refused. `toggleStar` sends `plan.starred` / `plan.participant`.
  - `wa_ui::chat_actions::mute_plan(choice)` — the menu's `0` / `-1` /
    `28800000` / `604800000` choices become `{ until_ms, forever, toast }`,
    including the `8 hours` vs `1 week` copy and a safe week fallback for an
    unreadable value (it used to fall through to `NaN`). `entry.muteForever`
    now follows `plan.forever` instead of re-deriving `until < 0`.
  - `wa_ui::state::contact_sync_needed(syncing, known)` — the address book is
    requested once per connection and only while nothing is stored.
  Commands `ui_star_plan`, `ui_mute_plan`, `ui_contact_sync_needed`.
- 3 new `wa-ui` unit tests (215 total, plus 3 contract) and a
  star/mute/contact-gate CDP e2e section: the three star plans, the five mute
  choices (including the garbage fallback), the gate's three states, and a live
  `applyStar` that flags the message and restores it.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.40] — 2026-11-03

- **The status composer's draft rules moved to Rust (`wa_ui::status`).**
  - `own_update(input)` builds the optimistic own story after a successful post:
    it keeps the **server id** when the post returned one (only that id can be
    revoked), falls back to the client id for local media (which the local media
    map is keyed by) and then to `local-<timestamp>`, and always marks it
    `from_me` + `viewed` with the `me`/`Me` identity fallbacks. Both post paths
    (text and media) and the media composer's text fallback use it.
  - `draft_preview(mime, url)` returns `{ kind, html }` — `<video controls>` for
    videos, `<img>` for images, nothing otherwise — so the composer's preview
    markup is escaped and built in Rust. A picked file that cannot be previewed
    is now **dropped from the draft** (it used to linger as `statusDraftFile`
    until the post-time check rejected it).
  Commands `ui_status_own_update`, `ui_status_draft_preview`.
- 2 new `wa-ui` unit tests (212 total, plus 3 contract) and a status-composer
  CDP e2e section: the server/local/keyed ids, the three preview kinds (including
  a quote-injection URL that cannot break out of the attribute), and a live
  composer that previews a real image then clears the draft for a PDF.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.39] — 2026-11-03

- **The history re-sync rules moved to Rust (`wa_ui::history`).**
  - `sync_progress(done, total)` → `{ percent, text, count, complete }`: the bar
    fill (rounded, capped at 100), `Syncing chat history… 3/8`, then
    `History refreshed`, and an empty counter for an empty batch. A late event
    can no longer push the bar past 100 %; `showSyncProgress` is async and only
    paints.
  - `sync_allowed(syncing, connected, last_at, now, force)` — the gate that used
    to be three inline conditions: never while a re-sync runs or offline, and at
    most once a day unless forced (a force still loses to "busy").
  - `HISTORY_SYNC_BATCH` (8) and `HISTORY_SYNC_WINDOW_MS` (24 h) are Rust
    constants mirrored at boot via `ui_history_sync_constants`, so the pacing
    knobs live in one place.
  Commands `ui_sync_progress`, `ui_sync_allowed`, `ui_history_sync_constants`.
- **The rail notification badges are Rust (`wa_ui::sidebar::rail_badges`).**
  Which chats count toward the pill (channels, the account's own chat and hidden
  deleted chats are skipped), the 99 cap, the `dot` for unseen stories and the
  change-guard key all come from one call; `updateRailBadges` only applies it.
  Command `ui_rail_badges`.
- 3 new `wa-ui` unit tests (210 total, plus 3 contract) and a
  history-sync/rail-badges CDP e2e section: the five progress states, the six
  gate outcomes plus the constants, the badge filtering/cap, and a live pass
  that paints `38%` → `100%` and shows a pill matching the store's unread count.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.38] — 2026-11-03

- **The group-screen rules moved to Rust.**
  - `wa_ui::labels::group_member_count` — `Group - 34 members` / `Group - 1 member`
    (the group info subtitle used to be a template literal).
  - `wa_ui::menus::member_menu_position` — the member kebab's menu hugs its row
    (right edge 20 px past the button) and flips up near the bottom, replacing
    two hand-written `Math.max/Math.min` lines; `openMemberMenu` is async now.
  - `wa_ui::chat_actions::member_action` — promote / demote / remove each carry
    their Tauri command, the confirmation copy (`Remove this member from the
    group?`), the toast and whether the list refreshes. `applyMemberAction`
    just runs the plan, and an unknown action or a missing target does nothing.
  - `wa_ui::picker::group_form_error` — the New group / Add members form
    validation (`Give the group a name.`, `Pick at least one participant.`),
    with the name checked first and no subject needed when adding members.
  Commands `ui_group_member_count`, `ui_member_menu_position`,
  `ui_member_action`, `ui_group_form_error`.
- 4 new `wa-ui` unit tests (207 total, plus 3 contract) and a group-plans CDP
  e2e section: the two count labels, the menu placement (normal + near the
  bottom), the five action outcomes, the form errors, and a live group screen
  whose member kebab opens with Promote hidden for an admin row.
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.37] — 2026-11-03

- **Reactions moved to Rust (`wa_ui::reactions`).**
  - `toggle(my, tapped)` → `{ value, revoke_first, mine }`: tapping the same emoji
    removes our reaction, a different one replaces it *and* revokes first (the
    server does not apply a plain replacement consistently), nothing yet is a
    plain add. `applyReaction` only performs the sends it is told to.
  - `after_send(rows, value, now)` replaces our own pill and leaves everyone
    else's in place (or drops ours when the value is empty).
  - `apply_event(rows, by, emoji, ts)` folds a live `reaction` event: an older
    timestamp than the row we hold is a delayed echo and is ignored, an empty
    emoji removes that author's pill, and other authors are untouched.
  Commands `ui_reaction_toggle`, `ui_reaction_after_send`, `ui_reaction_event`;
  the `case "reaction"` handler in `calls.js` is now three lines.
- **Receipts are applied as one batch.** `wa_ui::incoming::receipt_plan(ids,
  incoming, messages)` answers which bubbles may advance (a receipt never rolls
  a state back, and a missing status counts as `sent`), so the receipt handler
  makes **one** IPC call per receipt instead of one per matching message.
  `ui_receipt_status` is gone with its last caller, replaced by
  `ui_receipt_plan`.
- 2 new `wa-ui` unit tests and one expanded test (203 total, plus 3 contract) and
  a reaction/receipt CDP e2e section: the three toggle outcomes, our own row
  replacement/removal, the live-event fold (fresh / stale / removal), the batch
  receipt filtering, and a local path proving a failed send leaves the pill
  untouched. A live probe on the real account tapped 👍 on a message and tapped
  it again: `me:👍` → `[]`.
- Suite robustness: the incoming-rules section now deletes its probe chat first
  (a crashed run had persisted it, which made the unread/fold checks fail).
- Audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.36] — 2026-11-03

- **The chat scroll bookkeeping moved to Rust.** `wa_ui::scroll` now owns:
  - `scroll_event(last_top, top, distance)` → `{ moved_up, stick_bottom,
    clear_unread }`: moving up detaches the chat from the newest message,
    reaching the bottom re-attaches it and clears the disc's unread dot, and a
    1 px jitter is not a move. `wiring.js`'s scroll handler sends the metrics it
    already read and applies the answer under a token, so a newer scroll always
    wins over a slower reply.
  - `visible_anchor(nodes, scroll_top)` → the first bubble still visible at the
    top edge (with its offset), replacing `topVisibleMessage`'s DOM loop; the
    repaint anchor now comes from one batched call.
  - `latest_button_offset(heights, gap)` and the new
    `LATEST_BUTTON_BOTTOM_GAP_PX` (19) — the "jump to latest" disc lifts above
    whatever composer cards (reply / edit / draft) are visible, with the stack
    sum and resting gap computed in Rust.
- **New audit: `scripts/check-state-fields.mjs`** — lists fields declared in the
  `state` literal but never referenced. Currently **0 of 38**, so the store has
  no leftovers from the ports.
- 3 new `wa-ui` unit tests (199 total, plus 3 contract) and a scroll CDP e2e
  section: the four event plans, the anchor lookup (including the 1 px edge and
  "past everything"), the disc offset, and a live detach → disc + unread dot →
  re-attach round trip.
- All audits: `check-undefined` 21 (false positives), `check-ids` 0/271,
  `check-data-attrs` 0/84, `check-command-args` 0, `check-dead-functions` 0,
  `check-state-fields` 0/38.

## [0.2.0-rc.35] — 2026-11-03

- **The story viewer's chrome state moved to Rust.** `wa_ui::status::viewer_chrome`
  answers, for one frame, which arrows are usable, whether this is your own story
  (reply bar and reaction chips hidden, the "add" badge shown, the view counter
  visible with its count) and the pause button's visibility/glyph/title.
  `ui_status_chrome` replaces the hand-rolled checks in `updateStatusNav`,
  `statusReplyVisible` and `updateStatusPauseButton`; `renderStatusViewCount` is
  folded into that painter (its count and label now come from the same answer).
- **Stepping the queue is Rust too.** `wa_ui::status::next_position` returns the
  next `StepTarget { group, frame }`, wrapping into the next group when a group
  ends and reporting `None` at the first/last frame (the viewer closes on the
  last one). A backwards step into the previous group returns `frame: null` —
  "the last frame of that group" — instead of a `usize::MAX` sentinel, which
  serialized as `18446744073709552000` and was a latent precision bug.
  Commands `ui_status_chrome`, `ui_status_next_position`.
- 2 new `wa-ui` unit tests (196 total, plus 3 contract) and a story-viewer CDP
  e2e section: the own/theirs/empty chrome, the five step targets (including the
  group wrap and both ends), and a live viewer that pauses (`Play`, pressed
  glyph), resumes and steps.
- All five audits re-run: `check-dead-functions` 0, `check-undefined` 21 (all
  false positives), `check-ids` 0/271, `check-data-attrs` 0/84,
  `check-command-args` 0.

## [0.2.0-rc.34] — 2026-11-03

- **Dead-code sweep with a new audit.** `scripts/check-dead-functions.mjs` lists
  every top-level UI function with no caller; six were leftovers from renderers
  that moved to `wa-ui` and are gone: `core.js` `ensureDayLabels` / `dayKey` /
  `clockLabel` (plus the four day-label caches they used), `status.js`
  `latestStatus` / `statusUpdateById`, `message-actions.js` `clampMessageMenu`.
  The audit now reports **0**.
- **Two handlers that were never wired are now live:**
  - `queueGroupMeta` was defined but never called, so regular group chats never
    fetched their metadata — no community nesting and no group subject in the
    sidebar. `sidebarRequest()` now queues every group (its own guards keep a
    repeated render cheap); the probe went from parent-only metas to 10 with
    communities nesting correctly.
  - `refreshThemeLabels` was never called, so the Rust theme labels were unused
    after boot. `main()` now refreshes them, which makes
    `ui_theme_label`/`ui_theme_resolve` the source of truth for the Settings row
    and the theme dialog.
- Webview JS: **9,604 lines** (from 9,657) with no behaviour lost — the sweep
  only removed unreachable code and connected two dead wires.
- 1 new CDP e2e section (dead-code/group-meta) and the four audits re-run:
  `check-dead-functions` 0, `check-undefined` 21 (all false positives),
  `check-ids` 0/271, `check-data-attrs` 0/84, `check-command-args` 0.

## [0.2.0-rc.33] — 2026-11-03

- **The document file-card helpers moved to Rust (and the dead ones are gone).**
  `wa_ui::chat::document_summary` returns the name, extension, tone, badge label,
  `previewable` flag and the `2 pages · PDF · 2.4 MB` line for one attachment;
  `ui_document_summary` backs the viewer and the remaining lookups.
  `media-ui.js` lost `DOC_MIME_EXTENSIONS`, `DOC_TONES`, `DOC_PREVIEWABLE`,
  `extensionOf`, `documentInfo`, `documentBadge`, `formatBytes`, `documentMeta`
  and `documentCard` — the last three were already dead because
  `wa_ui::chat::render` draws the card and its actions. `media-ui.js` is down to
  **833 lines** (from 925).
- **Download errors are explained in Rust.** `wa_ui::media::friendly_error`
  turns a 403/404/410 into "WhatsApp no longer serves this file…" and a timeout
  into "The download timed out…", leaving anything else untouched
  (`ui_friendly_media_error`).
- 2 new `wa-ui` unit tests (194 total, plus 3 contract) and a document-summary
  CDP e2e section: the PDF card line, the mime-derived name, a non-previewable
  type, the three error mappings and a live document viewer opening on a real
  `.xlsx` with the Rust badge (`doc-sheet`).

## [0.2.0-rc.32] — 2026-11-03

- **Fixed: the delete dialog never opened.** `askDelete` sent `self_chat` while
  Tauri binds command arguments as camelCase (`selfChat`), so
  `ui_delete_confirm` rejected every call and the dialog silently returned —
  Delete for me / for everyone did nothing. Found by a new audit script
  (`check-command-args.mjs`) that compares each `invoke("x", { … })` top-level key
  against the Rust command's parameter names; it now reports **0**.
- **The message menu is a Rust dispatch table.**
  `wa_ui::chat_actions::message_action` maps every row to an action kind, closes
  the menu for Forward, refuses `message-sender`/`reply-privately` without an
  author and owns the "Ask Meta AI isn't available…"/"Reporting isn't
  supported…" copy. `forward_payload` / `delete_targets` build the multi-select
  payloads (id + text; author only for someone else's message) and
  `copy_text` rejects blank bodies. Commands `ui_message_action`,
  `ui_forward_payload`, `ui_delete_targets`, `ui_copy_text`.
- **Status composer rules in Rust.** `wa_ui::composer::can_post_text` enables the
  text-send button only for non-blank text and `wizard_back` owns the Esc order
  (emoji panel → text screen → composer). Commands `ui_can_post_text`,
  `ui_status_wizard_back`.
- 4 new `wa-ui` unit tests (192 total, plus 3 contract) and a message-menu /
  composer CDP e2e section: the whole dispatch table, the author guards, the
  unsupported toasts, copy/composer gates, the back stack, both payloads and a
  live "selection → delete dialog opens" check that would have failed before the
  fix.

## [0.2.0-rc.31] — 2026-11-03

- **The AI settings form is validated in Rust.** `wa_ui::settings::ai_save_plan`
  trims every field, rejects a blank or non-`http(s)` base URL and a blank model,
  and turns an empty key field into `apiKey: null` so the stored key survives —
  returning the exact `save_ai_settings` payload. `ai_toggle` gates enabling on a
  stored key (`Add an API key first`) while disabling never needs one;
  `ai_require_toast` owns the `/ai` toggle copy; `ai_preview` requires an open
  one-to-one chat and falls back to the sample prompt; `toast_excerpt` cuts long
  model replies to 100/120 chars + `…`; `profile_name_error` rejects a blank
  display name. Commands `ui_ai_save_plan`, `ui_ai_toggle`,
  `ui_ai_require_toast`, `ui_ai_preview`, `ui_toast_excerpt`,
  `ui_profile_name_error`.
- 3 new `wa-ui` unit tests (188 total, plus 3 contract) and an AI-form CDP e2e
  section: the shaped payload, the URL rejection, the three toggle outcomes, both
  requirement toasts, the preview guard/sample, the truncated excerpt, the blank
  name, and a live AI-mode toggle round-trip on the settings page.
- Audit scripts re-run on the built app: `check-undefined` 22 (all false
  positives: `async`/`scaleX`/words inside template strings), `check-ids` 0
  missing of 271, `check-data-attrs` 0 unemitted of 84.

## [0.2.0-rc.30] — 2026-11-03

- **The chat merge and the optimistic text bubble moved to Rust.**
  `wa_ui::state::fold_plan` decides which of *rename in place / re-key / merge*
  a peer lookup triggers, which source ids the target is missing, whether the
  source's `lastTs` wins, how much unread to add, and that a verified name only
  replaces a row still showing the bare JID user part.
  `wa_ui::actions::outgoing_message` shapes the optimistic send bubble —
  `pending`, `from_me`, the group flag, timestamp, text and the reply-quote card
  (`You` when we quoted ourselves). Commands `ui_fold_plan`,
  `ui_outgoing_message`; `foldChat` and `sendCurrent` only apply the results.
- **Data-attribute audit.** `scripts/check-data-attrs.mjs` cross-references every
  `dataset.x` / `[data-x]` read against the markup emitted by `index.html`, the
  Rust renderers and the JS templates, ignoring attributes the JS writes itself:
  **0 unemitted** of 84 read (81 emitted, 26 runtime-only).
- 2 new `wa-ui` unit tests (185 total, plus 3 contract) and a chat-fold/outgoing
  CDP e2e section: the four fold modes, the plain and quoted bubbles, and a live
  merge where the source row disappears and the target ends up with both ids,
  summed unread, the newer `lastTs` and the verified name.

## [0.2.0-rc.29] — 2026-11-03

- **The chat-row store mutation moved to Rust.** `wa_ui::state::upsert_plan`
  decides what an event does to its chat row: create it with the JID user part as
  the fallback name, rename it only while the name is still a placeholder, and
  keep the group flag in sync. `insert_index` places a message in the
  timestamp-sorted list without re-sorting (equal timestamps keep arrival order,
  which is what an echo needs). Commands `ui_upsert_plan`, `ui_insert_index`;
  `upsertChat` is async now and the three call sites (chat event, channel info,
  community seeding) await it, and `addIncoming` splices at the reported index.
- **Element-id audit.** `scripts/check-ids.mjs` cross-references every
  `$("id")` / `getElementById("id")` against `index.html` plus the ids the
  renderers inject: **0 missing** out of 271 static ids, so that regression class
  (silent no-ops from renamed elements) is currently clean.
- 2 new `wa-ui` unit tests (183 total, plus 3 contract) and a chat-row
  upsert/insert CDP e2e section: create/rename/keep plans, the stable insert
  indexes, a live create-and-rename on a probe chat, and a live `addIncoming`
  landing at index 100 of a 100+ message thread with the list still sorted.

## [0.2.0-rc.28] — 2026-11-03

- **Three more undefined-handler crashes fixed.** A new audit script
  (`scripts/check-undefined.mjs`) scans every call site against the definitions
  and found: `toggleSideMenu` / `closeSideMenu` (the sidebar kebab threw, so the
  ⋮ menu never opened), `statusCaptureLimits` (posting any status photo/video
  threw) and `stripThumbInner` (refreshing a video poster threw, because that
  helper moved to `wa_ui::viewer`). All three are now implemented:
  - the kebab opens/closes and is positioned by the new
    `wa_ui::menus::anchor_menu` (right-aligned under the button, flipped up when
    the button is near the bottom, never outside the margin) via
    `ui_menu_anchor`;
  - `statusImageThumb` uses `ui_status_capture` + `ui_status_thumb_scale` for its
    box and JPEG quality;
  - `applyThumb` renders filmstrip cells through `ui_viewer_strip_thumb`.
- The e2e suite now asserts that nine previously-missing handlers are defined, so
  this class of regression fails the run instead of only breaking at runtime.
- 1 new `wa-ui` unit test (181 total, plus 3 contract) and a
  menus/missing-handler CDP e2e section: the anchor maths (including the flip-up
  case), the live kebab (6 items, positioned, closes) and the capture limits.

## [0.2.0-rc.27] — 2026-11-03

- **Fixed: "Select chats" threw a `ReferenceError`.** The sidebar menu and the
  row click handler called `startChatSelection` / `exitChatSelection` /
  `toggleChatSelection` / `applyChatSelection` / `markAllChatsRead`, but none of
  them existed — the `#chat-select-bar` markup was dead. The handlers are now
  implemented and the selection rules come from Rust: `ui_toggle_selection`
  (shared with the message selection) and `ui_selection_bar` for the counter and
  the button states.
- **Bulk sidebar actions are planned in Rust.** `wa_ui::chat_actions::bulk_chat_plan`
  maps `read` / `archive` / `delete` over the selected chats, skipping rows that
  would not change (a read chat for `read`, an archived one for `archive`; delete
  never skips and always asks), and returns the same `ChatActionPlan` shape the
  single-row menu uses. Command `ui_bulk_chat_plan`.
- `markAllChatsRead` now walks the chats with an unread badge and reports how
  many it marked instead of silently doing nothing.
- 1 new `wa-ui` unit test (180 total, plus 3 contract) and a sidebar
  select-chats CDP e2e section: the bulk plans, the live selection mode (bar,
  counter, ticked row, enabled actions), two rows selected, and exit when the
  last row is unticked.

## [0.2.0-rc.26] — 2026-11-03

- **The profile-photo pump, the theme rules and the avatar/media icon tables
  moved to Rust.** `wa_ui::state::photo_pump` fills the fetcher up to its
  concurrency window (3) and reports what is still waiting;
  `photo_result_ok` keeps only real CDN URLs in the cache (a `blob:`/`data:`
  URL would be persisted and fail to paint on the next start).
  `wa_ui::settings` owns the theme labels, `resolve_theme` (explicit choice beats
  the OS) and the toggle's label/icon. `wa_ui::html::avatar_palette` supplies the
  11 official tints, `GROUP_AVATAR_ICON` the two-person glyph and
  `wa_ui::chat::media_icon` the media glyphs — so `core.js` lost the colour
  table, the ~750-byte inline SVG and the five-icon media table.
  `wa_ui::emoji::active_group` decides the scrolling category strip.
  Commands: `ui_photo_pump`, `ui_photo_result_ok`, `ui_theme_label`,
  `ui_theme_resolve`, `ui_theme_toggle`, `ui_avatar_palette`,
  `ui_group_avatar_icon`, `ui_avatar_style`, `ui_initials`, `ui_is_self_jid`,
  `ui_media_icon`, `ui_emoji_active_group`.
- The synchronous call sites keep a boot-fetched mirror (palette, group glyph,
  thresholds) and the emoji strip asks Rust on a 50 ms debounce rather than per
  scroll event; `requestAnimationFrame` was rejected because it is suspended
  while the window is occluded, which left the debounce flag stuck.
- 4 new `wa-ui` unit tests (179 total, plus 3 contract) and a
  photo-pump/theme/icons CDP e2e section (pump windows, http-only cache, strip
  maths, theme labels/resolution/toggle, 11 tints, `AL` initials, 194 painted
  sidebar avatars, live theme toggle).

## [0.2.0-rc.25] — 2026-11-03

- **The call codec and audio constants moved to Rust.** `wa_ui::calls` now owns
  the H.264 profile (`avc1.42E01F`), 640×480 @ 15 fps / 700 kbps with Annex-B
  and the 30-frame keyframe interval (`ui_video_config`, `ui_is_keyframe`), the
  16 kHz / 960-sample (60 ms) audio frame (`ui_audio_format`), the float→i16
  conversion (`ui_pcm_sample`) and the jitter-buffer window: start 60 ms out,
  reset when the peer is under 20 ms behind or over 600 ms ahead
  (`ui_audio_schedule`). `calls.js` fetches the config once and its encoder,
  decoder and audio scheduler read it; `isKeyframe` uses the Rust interval.
- **The in-chat scroll maths moved to Rust.** `wa_ui::scroll` has
  `distance_from_bottom`, `is_following` (120 px), `show_latest_button` (needs
  content and a > 8 px gap) and `anchor_scroll_top` (never negative, falls back
  to the previous position when metrics are missing). Commands
  `ui_scroll_distance`, `ui_scroll_following`, `ui_scroll_latest_visible`,
  `ui_scroll_anchor` and `ui_scroll_constants` (so the synchronous scroll
  handler mirrors the thresholds without awaiting per event); the anchored
  repaint asks Rust where to land.
- 6 new `wa-ui` unit tests (175 total, plus 3 contract) and a codec/scroll CDP
  e2e section: the video config, the keyframe cadence, the audio frame, the
  jitter-buffer resets, symmetric PCM, the scroll thresholds and a live chat
  that still sticks to the newest message (221 bubbles, `stickBottom = 1`).

## [0.2.0-rc.24] — 2026-11-03

- **The composer's media-send pipeline is planned in Rust.**
  `wa_ui::actions::draft_send_plan` decides that only the **first** file of a
  media draft carries the composer text as its caption (trimmed), returning one
  `{ index, caption }` per file, so `sendDraft` no longer builds that rule
  itself. `attachment_message` shapes the optimistic bubble an upload shows —
  `pending`, `from_me`, group flag, timestamp and, for documents, the same file
  card an incoming document gets (name, MIME, size, `pages: 0`, no thumb) — and
  `attachment_result` says what the bubble becomes afterwards: the server id
  only on success, its temp id and `failed` on error. Commands
  `ui_draft_send_plan`, `ui_attachment_message`, `ui_attachment_result`.
- 3 new `wa-ui` unit tests (169 total, plus 3 contract) and a composer-send CDP
  e2e section: the caption plan, both optimistic bubbles, the success/failure
  results and a live staged draft (one image in the strip, an empty `sendDraft`
  leaves the chat untouched).

## [0.2.0-rc.23] — 2026-11-03

- **Media-hydration decisions moved to Rust.** `wa_ui::media` answers, for each
  rendered tile, what `hydrateMedia` must do: `skip` (a failed tile waits for a
  tap), `repaint` (cached and ready — the Rust renderer owns the markup),
  `retry_tap`, `tap_to_load` (chat videos never autoload a long clip) or
  `observe` (images, channel videos, audio, documents). One batched
  `ui_hydrate_plan` call covers a whole repaint instead of a rule chain per
  tile. `ui_download_action` refuses a start while a tile is `loading` **or**
  already `ready`, and `ui_evictions` lists the oldest cache keys an eviction
  pass drops (thumbnails 240, sampled posters 120).
- **The multi-select toggle is Rust.** `wa_ui::actions::toggle_selection`
  returns the new set, whether the toggled bubble ended up selected and whether
  the set emptied (which ends select mode) — `ui_toggle_selection`.
- Commands: `ui_hydrate_plan`, `ui_download_action`, `ui_evictions`,
  `ui_toggle_selection`.
- 7 new `wa-ui` unit tests (166 total, plus 3 contract) and a
  media-hydration/selection CDP e2e section: the six-action plan, the download
  guard, the eviction list, the toggle sequences, and the live paths (127 tiles
  hydrate a download; the bar reads `2 selected`; the last toggle exits).

## [0.2.0-rc.22] — 2026-11-03

- **Status view receipts and interaction healing moved to Rust.**
  `wa_ui::status::note_view` normalizes the author JID (a device suffix like
  `111:42@lid` becomes `111@lid`), refuses the `@broadcast`/`@newsletter`
  pseudo-viewers and drops a repeat, returning the row to store.
  `heal_plan` decides which replies must wear the "· Status" card (their quote
  target is a story we track) and which phantom chat bubbles (a story recovered
  through a reply quote) are dropped, per chat. `wa_ui::text::jid_without_device`
  is the shared normalizer. Commands `ui_status_note_view`, `ui_status_heal`;
  `noteStatusView` and `healStatusInteractions` only apply the results.
- **The image-draft stack is planned in Rust.** `wa_ui::actions::draft_plan`
  owns `MAX_DRAFT_FILES` (10) and the image-only gate, returning the accepted
  indices plus the first toast, so `addDraftFiles` no longer loops over rules.
  Command `ui_draft_plan`. The status add-menu now uses the shared
  `ui_clamp_popover` too (it had a fourth copy of the edge maths).
- 3 new `wa-ui` unit tests (159 total, plus 3 contract) and a
  status-views/draft CDP e2e section: normalized + deduped receipts, the heal
  plan, the capped image-only draft stack, and the live paths (one accepted
  image of two offered, a healed reply, a recorded viewer).
- Contract note: `ui_status_heal` takes `updateIds` (Tauri camel-cases command
  arguments); the first probe failed on `update_ids` and was corrected.

## [0.2.0-rc.21] — 2026-11-03

- **The status viewer's group queue is planned in Rust.**
  `wa_ui::status::viewer_groups` applies the 24-hour expiry, orders the groups by
  their newest activity and returns each group's update ids oldest first (the
  play order), so `openStatusViewer` only maps ids back to the local updates it
  already holds. `own_update_ids` lists our own stories, which replaces the
  hand-rolled scan in `syncOwnStatusIds`.
  Commands: `ui_status_groups`, `ui_status_own_ids`.
- **Popover clamping moved to Rust.** `wa_ui::menus::clamp_popover(x, y, width,
  height, viewport, margin)` keeps the chat menu, its mute submenu and the
  message menu 8–10 px inside the window (taller menus are still pushed negative
  so they scroll instead of overflowing). `ui_clamp_popover` is shared by
  `clampMenu` and `clampMessageMenu`, replacing three copies of the min/max
  maths.
- 3 new `wa-ui` unit tests (156 total, plus 3 contract) and the existing
  status/context-menu CDP e2e sections still pass with the new code paths.

## [0.2.0-rc.20] — 2026-11-03

- **The call overlay is driven by Rust.** `wa_ui::calls` decides which controls
  show (accept/reject while ringing, mute/hangup after), whether the video stage
  is up (only once answered), whether the self view is painted (hidden on a
  camera-less video call so no black tile), the muted button state and the
  status line — `Incoming video call` / `Incoming voice call` / `Calling...` /
  `Connected · 01:05`. It also owns the outgoing-call guard (`Group calls aren't
  supported yet.`, `A call is already active.`) and the missing-media toasts.
  Commands: `ui_call_overlay`, `ui_call_guard`, `ui_call_media_toast`;
  `showCallOverlay` and the per-second timer now read the Rust status instead of
  formatting `mm:ss` themselves.
- **The status composer's capture limits are Rust.** `ui_status_capture` returns
  the 320 px thumbnail box and 0.72 JPEG quality, and
  `ui_status_duration_seconds` rounds a posted video's length (minimum 1 s).
- 5 new `wa-ui` unit tests (153 total, plus 3 contract) and a calls/capture CDP
  e2e section: the ringing and connected overlays, the guards, the capture
  limits and a live overlay driven through `showCallOverlay`.

## [0.2.0-rc.19] — 2026-11-03

- **The disk chat-list mirror and the address-book renames are Rust decisions.**
  `wa_ui::cache::restore_disk` shapes the rows recovered from `chats.json`
  (broadcast rows dropped, JID user part as the fallback name, a preview
  synthesized from `lastText`/`lastTs`), so `restoreDiskChatCache` only inserts
  them. `wa_ui::state::contact_rename_plan` decides which one-to-one chats take
  a saved name (never the account's own chat) and which sender bubbles in any
  chat get relabelled; `state-updates.js` applies the plan and now batches the
  address-book replay into one call per 150 ms burst instead of one call per
  contact.
- Commands `ui_restore_disk_cache`, `ui_contact_plan`.
- 2 new `wa-ui` unit tests (148 total, plus 3 contract) and a restore/contacts
  CDP e2e section: the restored rows, the rename plan and a live rename applied
  to a probe chat and its sender bubble.

## [0.2.0-rc.18] — 2026-11-03

- **AI context and sticker selection moved to Rust.** `wa_ui::context` builds
  the bounded transcript (visible messages only, newest 24, `Gw` for our own
  turns, sender name → user → `Mereka`, `[Photo]`-style placeholders for media,
  lines without an id/text dropped), dedupes the recent-sticker list (newest
  copy per id, no `local-` optimistic bubbles, no pending ones, capped at 80)
  and filters AI sticker ids to those whose media is cached.
  Commands `ui_ai_context`, `ui_recent_stickers`, `ui_ai_sticker_ids`.
- **Media hydration limits moved to Rust.** `ui_media_limits` returns the cache
  budgets (240 thumbnails, 120 sampled posters), the 12 s download/watchdog
  timeout, the 1.2 s sticker timeout, the 300 px observer margin, the poster
  width/quality/timeout; `ui_poster_seek` (8% into the clip, clamped 0.05–1.5 s)
  and `ui_poster_height` (aspect-correct, 180 px fallback) drive frame sampling.
  `stickers.js` and `media-ui.js` now read those instead of their own constants.
- 5 new `wa-ui` unit tests (146 total, plus 3 contract) and a
  context/hydration CDP e2e section: limits, poster maths, the labelled
  context, sticker dedupe, the cached-only AI filter and the grid still
  rendering (37 cells).

## [0.2.0-rc.17] — 2026-11-03

- **Composer, seeding and peer-alias plans moved to Rust.**
  `wa_ui::actions::send_plan` decides what the send button does (channel =
  blocked with its toast, media draft, edit, send, or a no-op for empty text);
  `wa_ui::state::seed_plan` picks the address-book entries that rebuild personal
  chats (≥6 digits, not our own number, not already known, digits only);
  `wa_ui::state::peer_fold_plan` decides which LID rows fold into the
  phone-number row and which placeholder names the verified identity replaces.
  Commands `ui_send_plan`, `ui_seed_chats`, `ui_alias_plan`; `actions.js` and
  `chats.js` only apply the results (`foldChat` keeps doing the message merge).
- 5 new `wa-ui` unit tests (141 total, plus 3 contract) and a store-plans CDP
  e2e section: send `send/noop/blocked/draft/edit`, the filtered seed list and
  a LID→PN fold with the rename.
- Contract note: Tauri camel-cases command args (`mePhone`), caught by the probe
  before the e2e run.

## [0.2.0-rc.16] — 2026-11-03

- **Chat-menu actions are planned in Rust.** `wa_ui::chat_actions::plan_chat_action`
  maps a menu row to the wire command and its args (`chat_pin`,
  `chat_archive`, `chat_read_mark`, `chat_block`, `chat_clear`, `chat_delete`),
  the local mutation (pin/archived/unread/blocked/clear/delete/close), the
  confirmation copy for destructive rows and the toast. Unknown rows plan
  nothing. `runChatAction` only executes the plan; the dead local `applyPin`
  helper is gone.
- **Delete commands and message info come from Rust.**
  `delete_commands` builds one `delete_for_me`/`delete_for_everyone` call per
  selected message (participant/`from_me` only for "for me"); `message_info`
  builds the Type/Sent/From/Message ID/Status rows (status labels
  `Sending…`/`Failed`/`Read`/`Delivered`/`Sent`, sender label from name → number)
  plus the dialog body, so `showMessageInfo` no longer formats anything.
- Commands: `ui_chat_action`, `ui_delete_commands`, `ui_message_info`.
- 6 new `wa-ui` unit tests (136 total, plus 3 contract) and a chat-actions CDP
  e2e section (pin/unread/block/clear/close plans, both delete modes, and the
  5-row info dialog). The status-body e2e check now polls instead of sleeping.

## [0.2.0-rc.15] — 2026-11-03

- **The story viewer's frame clock moved to Rust.** `wa_ui::status` now owns the
  timings (text 10 s + 40 ms/char beyond 80 capped at 4 s, photo 8 s, video
  placeholder 20 s, failed media 3 s), the frame anchor (`total`, `remaining`,
  `ends_at`), elapsed time from the same clock, the pause freeze
  (`remaining = ends_at - now`, end cleared) and resume re-anchor (never zero),
  the progress fill ratio, the thumbnail capture box (320 px, never enlarging)
  and the video poster seek (min 0.2 s, duration/2) plus its 4 s timeout.
  Commands: `ui_status_timing`, `ui_status_frame_duration`,
  `ui_status_anchor`, `ui_status_elapsed`, `ui_status_pause_remaining`,
  `ui_status_resume`, `ui_status_progress`, `ui_status_thumb_scale`,
  `ui_status_poster_seek`. The per-rAF paint keeps only the linear scaleX it
  derives from the Rust anchor (an IPC call per animation frame would be
  wasteful).
- `status.js` dropped its timing constants and the pause/resume arithmetic;
  `resumeStatusFrame` now awaits the re-anchor so the bar cannot sample a
  half-reset clock.
- 6 new `wa-ui` unit tests (130 total, plus 3 contract) and a status-clock CDP
  e2e section: constants, durations, anchor 9000/elapsed 4000, pause 5500,
  resume ≥1 ms, ratio 0.5, thumbnail 320×240, seek 0.2 and a live
  pause/resume of the viewer.

## [0.2.0-rc.14] — 2026-11-03

- **Incoming-message application rules moved to Rust.** `wa_ui::incoming`
  decides what a message event does: fold into an existing id (key-only stub
  upgrade, channel stats, forwarded marker, link thumbnail), fold our own echo
  into the newest optimistic bubble (same kind/text within two minutes), or
  append. It also owns the unread rule (open chat clears, own messages clear,
  background chats bump), receipt ordering (`sent < delivered < read`, so a
  delayed `sent` cannot undo `read`), the "Message yourself" delivery status,
  the newest-pending lookup and the AI echo guard (same text within 3 s).
  Commands: `ui_incoming_classify`, `ui_pending_echo`, `ui_unread_rule`,
  `ui_receipt_status`, `ui_self_chat_status`, `ui_newest_pending`, `ui_ai_echo`.
- **Event handling is serialized.** `handleEvent` is async (it asks Rust how to
  classify), and `main.js` now chains `wa:event` deliveries so two messages
  cannot interleave their awaits.
- 8 new `wa-ui` unit tests (124 total, plus 3 contract) and an incoming-rules
  CDP e2e section: unread bump/clear, duplicate folding without appending,
  send-echo folding, receipt advance-only and the AI echo guard. The sidebar
  search check was also made repaint-safe (polls for the mark).

## [0.2.0-rc.13] — 2026-11-03

- **Attachment validation and in-chat search planning moved to Rust.**
  `wa_ui::actions::attachment_check` owns the 48 MB limit and the
  image/video/document classification (with the exact refusal copy);
  `wa_ui::search` decides which rendered bubbles match a query
  (`matched_indices`, case-insensitive) and builds the counter label
  (`1/6`, `0/0`, or empty when the box is blank). Commands
  `ui_attachment_check`, `ui_search_matches`, `ui_search_label`.
  `attachments.js` keeps only the FileReader/optimistic-bubble wiring and
  `chat-search.js` only paints the text-node ranges.
- **The last duplicated helper is gone.** `state-updates.js` had its own
  `isPlaceholderName`; both call sites now use `ui_placeholder_name` (the
  rename also refreshes the sidebar when it lands).
- 2 new `wa-ui` unit tests (116 total, plus 3 contract) and a CDP e2e section:
  attachment classification + oversize refusal, planned matches `0,2`,
  labels `1/2` and `0/0`, and a live in-chat search (6 bubbles, `1/6`).

## [0.2.0-rc.12] — 2026-11-03

- **`icons.js` and `chat-utils.js` are gone.** The four remaining runtime icon
  swaps (`chat-render` phone glyph, the viewer's play/pause/mute buttons, the
  theme toggle) call `ui_icon`, which renders the Material SVG in Rust; the
  icon table no longer ships with the webview.
- The last `chat-utils` helpers moved to `wa-ui`: `aiPrompt` → `ui_ai_prompt`,
  `firstUrl` → `ui_first_url`, `isPlaceholderName` → `ui_placeholder_name`,
  `formatPhone` → `ui_format_phone` and `phoneFromJid` → `ui_phone_from_jid`
  in the async flows (calls, forward picker, group info, contact seeding,
  link previews). The sidebar now sends the newest six messages as `tail` and
  `wa_ui::sidebar::SidebarChat::last_visible` picks the preview, so
  `lastVisibleMessage` is Rust's job.
- One accessor stays in `core.js`: `jidUser(jid)` extracts the phone from a JID
  because the chat/contact maps are keyed by the address book's phone key.
- 1 new command left to verify: `ui_message_visible` (eye-toggle rule).
  `cargo test -p wa-ui` 114 + 3 contract, clippy/fmt clean, and the packaged
  CDP e2e passes (the group checklist initially returned 0 contacts because
  `knownLids()` became async; the spread now awaits it).

## [0.2.0-rc.11] — 2026-11-03

- **Status and message actions are guarded by Rust.** `wa_ui::actions` decides
  who may reply to / react to / delete a story (own stories only delete, others
  only reply/react, with the "Only your own status can be deleted." copy),
  validates a status draft (a media draft needs an `image/`/`video/` MIME with
  its caption trimmed; a text draft needs text) and builds the reply banner
  (author from saved contact → push name → `You`, text or media label preview
  capped at 140 chars, the participant colour) and the multi-select bar
  (`1 selected`/`N selected` + disabled state).
  Commands: `ui_status_permissions`, `ui_status_post_check`, `ui_reply_preview`,
  `ui_selection_bar`.
- `status.js` reply/react/delete/post paths and `message-actions.js`'s
  `startReply`/`updateSelectBar` now call those instead of re-implementing the
  rules.
- 5 new `wa-ui` unit tests (114 total, plus 3 contract) and an action-guards CDP
  e2e section (permissions, draft validation, reply preview, selection bar and
  the painted reply banner).

## [0.2.0-rc.10] — 2026-11-03

- **The remaining store flows moved to Rust.** `wa_ui::export` parses a
  WhatsApp "Export chat" `.txt` (line header with AM/PM and 2-digit years,
  continuation lines, bidi marks, system entries, `Waiting for this message`
  stubs, channel exports without a sender column) and returns an import plan:
  superseded `export-*` rows to drop (the phone sent the same message within the
  120 s window) and the parsed messages that are new. Commands
  `ui_parse_export`, `ui_import_plan`.
- `wa_ui::history` owns the on-demand anchor rule (oldest non-deleted,
  non-local message; empty chat = no anchor) and the re-sync order (empty chats
  first, then most recent, skipping deleted/channels) — `ui_history_anchor`,
  `ui_history_targets`. `requestChatHistory` now uses the same anchor instead of
  a second copy of the rule.
- `wa_ui::cache::disk_payload` builds the disk mirror metadata (newest first,
  capped, 120-char preview) — `ui_disk_cache_payload`; `persistChatCache`
  awaits it.
- `state-updates.js` 530 → 472 lines, `actions.js` 488 → 500 (the async
  wrappers) with the parsing/ordering rules gone; 9 new `wa-ui` tests
  (110 total, plus 3 contract) and a flows CDP e2e section (anchor, targets,
  25 KB disk payload, export continuation, 2 planned adds).
- **Mute state moved to Rust too.** `ChatMenuInput` now carries the raw
  `mute_until`/`mute_forever`/`now` and computes `muted()`; `ui_chat_menu` and
  `ui_mute_menu` use it, and `ui_muted_chats` batch-lists the JIDs the
  notification pump must silence. `chat-menu.js` no longer calls
  `WAChatUtils.chatIsMuted`, and `syncNotifyState` sends the fields.

## [0.2.0-rc.9] — 2026-11-03

- **The webview cache layer moved to Rust.** `wa_ui::cache` owns the change
  fingerprint (the same 31-hash format, over compact per-message summaries), the
  persisted payload (newest chats first, `MAX_CHATS`, the last N messages per
  chat, `link.thumb` stripped to `null`, only `http` photos kept) and the
  restore shape (drops `@broadcast` rows and empty protocol stubs, resets
  `pending`, falls back to the JID user part for a missing name, filters stale
  photo URLs). Messages are carried as opaque JSON, so fields a later build adds
  survive the round-trip.
- **The profile-photo queue is batched in Rust.** `ui_photo_queue` filters a
  render's candidate list (connected, never `0@`/`@broadcast`, skips cached/
  failed/queued) and de-duplicates it; `core.js` gained `queuePhotos()` and the
  sidebar/chat/channels/status/picker call sites use it.
- Commands: `ui_cache_fingerprint`, `ui_cache_payload`, `ui_cache_restore`,
  `ui_photo_queue`. `core.js` is now async in `saveNow`/`restoreCache`
  (fingerprint first, so the ~0.5 MB payload is only shaped when something
  changed).
- 6 new `wa-ui` unit tests (98 total, plus 3 contract) and a CDP e2e section:
  stable fingerprint, a 507 KB payload, 198 restored chats, stripped
  thumbnails and a deduped photo queue.

## [0.2.0-rc.8] — 2026-11-03

- **Chat-store housekeeping moved to Rust.** `wa_ui::state` decides which
  duplicate conversations fold (shared display name, PN over LID, richer copy
  wins, groups/channels/placeholder names never fold), which message ids move,
  the resulting unread/deleted/cleared/last-ts and the jid remap
  (`ui_merge_chats`); it also computes the cache trim (per-chat cap, then the
  global budget from the oldest chats while protecting the open one,
  `ui_trim_messages`). The webview only applies the plans to its `Map`/arrays,
  so no message data round-trips. `chat-utils.js` dropped `mergeChats` and
  `trimMessages` and the now-unused `chatSubtitle` (237 → 148 lines).
- 7 new `wa-ui` unit tests (92 total, plus 3 contract) and a CDP e2e section:
  a synthetic LID+PN pair folds into one chat with both messages and the summed
  unread count, and a trim plan never drops from the protected chat.
- Note: Tauri exposes command arguments as camelCase (`perChat`, `keepJid`);
  the trim call site was corrected after the first probe failed with
  `missing required key perChat`.

## [0.2.0-rc.7] — 2026-11-03

- **The status composer is driven by Rust.** `wa_ui::composer` owns the official
  20-colour background palette, the four font stacks and their wire names, the
  `0xAARRGGBB` conversion (`argb`, falling back to `#00a884` for bad input), the
  index wrapping and the composer's emoji panel (its own
  `data-status-emoji-group`/`data-status-emoji` markup, the
  "Smileys & People" title, search results and the `No emoji match` state).
  Commands: `ui_status_palette`, `ui_status_argb`,
  `ui_status_emoji_categories`, `ui_status_emoji_group`; `status.js` fetches the
  palette once and cycles through it.
- 5 new `wa-ui` unit tests (85 total, plus 3 contract) and a CDP e2e section:
  20 colours / 4 fonts, an opaque ARGB value, the background and font CSS
  variables applied, 9 categories / 185 emoji and a 7-result search, with no
  JavaScript errors.

## [0.2.0-rc.6] — 2026-11-03

- **The chat header comes from the renderer.** `ui_chat` now also returns the
  display name (`(You)` on the self chat) and the subtitle (`Group chat`, the
  formatted phone, `Message yourself`, or the channel follower count), so
  `chat-render.js` no longer computes either and a late LID→PN resolution just
  repaints.
- **Emoji recents are rendered by Rust.** `wa_ui::emoji::recents_html` +
  `ui_emoji_recents` build the row; `emoji.js` keeps only the localStorage
  list.
- **Dead JavaScript markup/formatting helpers removed.** `core.js` lost
  `avatarHtml`, `bodyHtml`, `replyQuoteHtml`, `statusQuoteHtml`, `linkify`,
  `applyWhatsAppFormatting`, `mentionName`, `previewOf`, `timeOf`, `stampOf`,
  `dayOf` and `chatDisplayName` (794 → 557 lines); `chat-list.js` lost
  `formatFollowers`; `chat-utils.js` lost `isChatVisible`,
  `deletedMessageText`, `previewOf`, `previewTick`, `fileSizeLabel` and
  `siteOf` (315 → 237 lines) because their Rust counterparts own those rules.
  `chat-utils.js` was also calling `isPlaceholderName` from four modules
  without the `WAChatUtils.` prefix — a latent ReferenceError — now fixed.
- Verified by 80 `wa-ui` tests and the CDP e2e (header for group/DM/channel,
  three emoji recents, no JavaScript errors).

## [0.2.0-rc.5] — 2026-11-03

- **The media viewer is rendered by Rust.** `wa_ui::viewer` builds the header
  label and `Yesterday at 11:49 am`, the mention-coloured caption, the stage
  (zoom layer or the custom player with play/seek/time/speed/mute/fullscreen)
  and the filmstrip window (15 before + current + 16 after, active thumb,
  video badges and loading tiles). `ui_media_viewer`, `ui_viewer_stage`,
  `ui_viewer_strip_thumb` and `ui_viewer_placeholder` expose it;
  `media-viewer.js` fell from 421 to 365 lines and lost its icon table and
  three markup builders (the runtime play/mute swap now uses `waIcon`).
- **The sticker picker grid is rendered by Rust.** `wa_ui::stickers` builds the
  Create tile, favourite/recent cells with their loading placeholders, the
  search filter, the empty states and the `(31)` count; `ui_sticker_grid` is
  the only path, and a finished download re-renders the grid instead of
  patching cells. `stickers.js` is down to 357 lines.
- 10 new `wa-ui` unit tests (79 total) and two new CDP e2e sections: the viewer
  opens on an image with a 6-thumbnail filmstrip and the picker renders 32
  cells / 31 images with the count badge, no JavaScript errors.

## [0.2.0-rc.4] — 2026-11-03

- **The Settings screen is rendered by Rust.** `wa_ui::settings` builds all
  eight pages (home, profile, account, privacy, chats, AI assistant, shortcuts,
  help) plus the header avatar, the update row (check/install states), the AI
  form with its escaped values, the four Chat switches and the credits list;
  `ui_settings` returns `{ page, title, html }`, so an unknown page falls back
  to home in one place. `settings.js` fell from 564 to 374 lines and lost
  `SOURCE_URL`/`LICENSE_URL`/`SETTINGS_TITLES` and eight markup helpers.
- **Menu rules live in Rust.** `wa_ui::menus` decides which rows the chat menu
  shows (pin/unpin, mute/unmute, archive/unarchive, unread/read, block only for
  DMs), the mute submenu, the message menu (edit only for own text, no
  forward/reply/copy/star once a message is gone, "Message <sender>" only in
  groups, the Star/Unstar label) and the delete dialog (`Delete N messages?`,
  "for everyone" only when every message is ours outside "Message yourself").
  Commands: `ui_chat_menu`, `ui_mute_menu`, `ui_message_menu`,
  `ui_delete_confirm`.
- 11 new `wa-ui` unit tests (69 total) and two new CDP e2e sections: all eight
  settings pages render (7 home rows, 4 switches, 16 credit links, the update
  button) and both context menus open with the right visible rows.

## [0.2.0-rc.3] — 2026-11-03

- **The status/stories screen is rendered by Rust.** `wa_ui::status` owns the
  rail list (24-hour expiry, recent/viewed split, `(You)`-style own row with its
  text/media thumbnail, unseen count and photo queue), the story viewer shell
  (progress segments, name, `Today at 8:44 pm`, the text-story background, the
  avatar and the viewer count), the media body (`<img>`/`<video>` or the
  `Loading status...`/`no longer available` states) and the `Viewed by N` list.
  `status.js` keeps the timers, the media download and the event wiring;
  `ui_status_list`, `ui_status_frame`, `ui_status_media`,
  `ui_status_media_error` and `ui_status_views` expose the renderers.
  Verified by 5 new `wa-ui` unit tests (51 total) and the CDP e2e, which opens
  the status screen (18 recent / 3 viewed rows) and the viewer (3 segments, an
  image body, no JavaScript errors).
- **The new-chat / forward picker is rendered by Rust.** `wa_ui::picker` builds
  the rows: the "Message yourself" entry, saved contacts with their formatted
  number, known chats (groups only while forwarding), LID de-duplication,
  name/number filtering, the A–Z sections, the typed-number row and the forward
  ticks. `new-chat.js` fell from 322 to 249 lines and `ui_picker` exposes it;
  5 more unit tests (56 total) and the CDP e2e (367 rows, 25 letter sections,
  "You" first, a typed +62 number gets its own row).
- **The group member checklist is rendered by Rust.**
  `wa_ui::picker::group_contacts` filters saved contacts (LID and self excluded),
  formats the numbers and keeps the ticked boxes across a repaint;
  `contacts.js` fell from 141 to 121 lines and `ui_group_contacts` exposes it.
  Verified by a unit test and the CDP e2e (287 contacts, a ticked row stays
  checked, no JavaScript errors).
- **The group-info member list is rendered by Rust.**
  `wa_ui::screens::group_members` resolves saved names, formats the numbers and
  marks admins; `group-info.js` only sends the member rows and injects the HTML
  (`ui_group_members`). Verified by a unit test and the CDP e2e (11 members,
  6 admin tags, no JavaScript errors).

## [0.2.0-rc.2] — 2026-11-03

- **The UI model moved into Rust.** A new `crates/wa-ui` crate owns the text
  logic that used to live in `apps/desktop/ui/`: WhatsApp inline markup
  (`*bold*`, `_italic_`, `~strike~`, ```monospace```), URL masking/linkify,
  `@mention` resolution (self → saved contact → known push name), previews with
  the official `You:`/`~ push`/photo-label rules, tick states, phone formatting,
  sender colours, placeholders, `/ai` parsing, initials, avatars and the
  clock/stamp/day labels (`8:44 pm`). It is unit-tested (`cargo test -p wa-ui`,
  34 tests) and exposed through `commands/ui.rs` (`ui_labels`,
  `ui_chat_subtitle`, `ui_emoji_search`, `ui_emoji_groups`, `ui_bootstrap`).
- **The emoji catalogue and picker moved into Rust.** `emoji-data.js` (2,300
  emoji in 9 groups, ~2.3k lines) is gone: `crates/wa-ui/src/emoji_data.rs` is
  generated from the old file by `scripts/gen-ui-data.mjs`, and the picker grids
  and category strip are rendered by `wa_ui::emoji` (`ui_bootstrap`). Search
  runs in Rust (`ui_emoji_search`), so the status emoji panel reads the same
  catalogue instead of a second copy.
- **The chat list is rendered by Rust.** `wa_ui::sidebar` takes the webview's
  chat state and returns the row markup: visibility and the archived view,
  unread/group filters, pinned-first ordering, community nesting with the
  announcement subgroup folded away, search split into Chats / Contacts /
  Groups in common, the archived banner, `(You)` on the self chat, mute/pin
  glyphs, badges and the photo-request queue. `chat-list.js` keeps only the
  state → request mapping and one `ui_sidebar` call (with a sequence guard so a
  slow response cannot repaint an old list).
- **Channels, the community screen and starred messages are Rust too.**
  `wa_ui::screens` renders all three from the same request (`ui_channels`,
  `ui_community`, `ui_starred`); `chat-list.js` shrank from 802 to 434 lines.
- **The message list is rendered by Rust.** `wa_ui::chat` builds every bubble
  from the webview's state: day dividers, sender runs and `first` tails, group
  sender names/avatars, reply and "· Status" quotes, albums (`album-2/3/4`, the
  `+N` cell), photo/video/audio/document/sticker blocks (including the
  voice-note waveform, duration and seek line), document cards and their
  View/Save-as row, embedded link previews, reactions, stars, `edited`, ticks,
  channel captions and view/forward/reaction counters, view-once placeholders
  and unsupported stubs. The webview keeps hydration (downloads, thumbnails,
  players) and scrolling; `chat-render.js` fell from 743 to 474 lines and
  `media-ui.js` from 1,048 to 914, and a finished download now repaints through
  the renderer instead of patching the DOM, so the media/audio markup exists
  only in Rust.
- **Live verification over CDP.** `tests/e2e/ui-rust.mjs` drives the running
  window (remote debugging on 9222) and checks the Rust layer end to end:
  `ui_bootstrap` returns 74 icons + 2,300 emoji in 9 groups, the sidebar paints
  193 rows with avatars/previews, search highlights and keeps its sections, the
  picker renders 2,300 buttons and 9 categories, emoji search returns matches,
  the status panel uses the same catalogue (9/185), the channels panel shows 5
  channels, a community screen renders, opening a chat paints 250 bubbles with
  5 day dividers, 221 bodies and 31 media blocks, and **no JavaScript error is
  logged**. Screenshots: `tests/e2e/artifacts/chat-rust.png`,
  `chat-media-rust.png`, `chat-sticker-rust.png`.
- **Contract tests for the JS ↔ Rust boundary.**
  `crates/wa-ui/tests/webview_contract.rs` feeds the literal JSON that
  `sidebarRequest()` sends (including extra message fields the renderer ignores)
  through the renderers, so a renamed field fails `cargo test` instead of
  silently emptying the sidebar.
- **Video calls answered without a camera keep the peer's video.** Answering a
  video offer used to pass `video: false` when the machine had no camera; the
  call library then built an audio-only engine, so the caller's frames were
  dropped and the window stayed black even though the phone was sending. The
  offer now decides (`negotiated_video(offered, requested)` in
  `wa-core/src/calls.rs`, the UI sends the offered flag), the decoder starts
  before the encoder, a camera that fails to open falls back to audio-only
  without hanging up, and the self-view hides instead of painting a black tile.
  Verified by `cargo test -p wa-core calls::tests` and by the webview's
  `startCallMedia(callId, video, sendVideo)` split.

## [0.1.0-rc.6] — 2026-11-03

- **The update channel now works for pre-releases.** GitHub's
  `/releases/latest` deliberately skips pre-releases, so the endpoint added in
  rc.5 answered **404** and every check failed. The release job now also
  publishes `latest.json` to a `latest` branch, and
  `plugins.updater.endpoints` lists that raw URL first, falling back to the
  release asset for builds that are not pre-releases.

## [0.1.0-rc.5] — 2026-11-03

- **In-app updates.** Settings → Account gained an **Updates** row. *Check for
  updates* asks `tauri-plugin-updater` whether a newer release is published;
  when one is, the row becomes *Update to vX*, which downloads the installer,
  verifies its minisign signature and installs it, then restarts into the new
  version. `plugins.updater.endpoints` points at the `latest.json` attached to
  the newest release, and `plugins.updater.pubkey` pins the key the CI signs
  with (`TAURI_SIGNING_PRIVATE_KEY`).
- **Settings → Help and feedback** now links the **source code** and the
  **licence**, shows the running version, and carries a **Special thanks** list
  crediting every library the client stands on (mirrored by the README's
  Credits section).
- The release job signs the installer (`bundle.createUpdaterArtifacts: true`),
  writes `latest.json` for the updater and uploads it alongside the other
  assets, so a tag push now produces a working update channel.
- **Security hardening before the repository went public.** `security.csp` was
  `null` — no Content Security Policy at all. It now denies `object`/`frame`,
  pins `connect-src` to the Tauri IPC, and allows only the image/media sources
  the UI actually uses; verified with **0 violations** in the webview console
  after a reload. The audit that produced this is written up in
  [`SECURITY.md`](SECURITY.md) — no secrets in the tree or the history, all
  commit emails are noreply, message text is escaped before `innerHTML`,
  `open_link` only accepts http(s), and the only `unsafe` is Win32/COM interop.
  CI now runs `cargo audit` on every push, with the accepted advisories and
  their rationale in [`.cargo/audit.toml`](.cargo/audit.toml).

## [0.1.0-rc.4] — 2026-11-03

- **The window now opens at a size that fits the screen.** The configured
  default is 1120x780, which is taller than a 1366x768 laptop once the taskbar
  is accounted for, so the composer was off-screen and the window had to be
  resized on every launch. `fit_window_to_work_area()` (`app.rs`) clamps the
  window to the monitor's work area — minus a 16x48 px allowance for the DWM
  frame and drop shadow, which Windows adds outside `outer_size` — and
  re-centres it. The window is created `"visible": false` and shown after the
  fit, so there is no resize flash. Locked by
  `app::tests::window_is_shrunk_only_when_it_does_not_fit`.
  - Verified live by temporarily configuring a 1100 px tall default:
    `window 1136x1100 against a 1920x1032 work area (max 1904x984) -> Some((1136, 984))`,
    with the real window rect `1152x1023 at (384,8)` — bottom 1031 against the
    work area's 1032.

## [0.1.0-rc.3] — 2026-11-02

- **The Account screen shows the real app version.** The `Build` row used to
  print the UI cache-bust number (`UI v141`); `get_status` now returns the app
  version from `tauri.conf.json` (`Status::version`), the webview stores it as
  `state.version`, and `buildLabel()` renders `<version> · WA Desk`.
- **Releases are published by CI.** A new `release` job in
  `.github/workflows/build.yml` runs on a `v*` tag: it builds the Windows MSI
  and NSIS installers, then creates the GitHub release and uploads the
  installers plus the plain binaries from the `build` matrix. The release body
  is the matching section of this file — the job extracts it with PowerShell —
  plus a diff link to the previous tag; the release is marked pre-release when
  the tag carries a pre-release suffix.

Full diff: [`68a19c0...v0.1.0-rc.3`](https://github.com/dirtTW/wa-desk/compare/68a19c0...v0.1.0-rc.3)

## [0.1.0-rc.2] — 2026-11-02

- **`aes-gcm` 0.10 → 0.11** — the bump deferred from #4. The 0.11 release
  removed `aead::OsRng` and `AeadCore::generate_nonce` and deprecated
  `Nonce::from_slice`, so `ai/state.rs` and `ai/util.rs` were migrated to the
  new `aead::Generate` trait (`<[u8; 32]>::generate()`,
  `AeadNonce::<Aes256Gcm>::generate()`) and to `TryFrom` for the decoded nonce.
  The on-disk format is unchanged — still
  `base64(12-byte nonce).base64(ciphertext)` — so an existing `ai.key` and any
  stored API key still decrypt. The Dependabot `ignore` for the aes-gcm major
  bump is removed now that the migration landed.

## [0.1.0-rc.1] — 2026-11-02

First pre-release: the client itself, the AGPL relicensing, the Windows
installers and CI.

- The app version is `0.1.0-rc.1`. The MSI `ProductVersion` is the numeric
  `0.1.0.1` because Windows Installer rejects a non-numeric pre-release
  identifier, so `bundle.windows.wix.version` pins it.
- **Dependency bumps (Dependabot):** `sha2` 0.10.9 → 0.11.1 (#3) and
  `base64` 0.22.1 → 0.23.1 (#2). The `aes-gcm` 0.10.3 → 0.11.1 bump is
  **deferred** — it needs an API migration in the AI key encryption
  (`generate_nonce` removed, `Nonce::from_slice` deprecated) and is tracked
  in #4.
- CI actions moved to `actions/checkout@v7` and `actions/upload-artifact@v7`
  (Node 24), and Dependabot now watches cargo, GitHub Actions and the e2e npm
  package.

Everything below is part of this release.

- **Bundle identifier is now `io.github.dirttw.wadesk`** (was the placeholder
  `dev.example.wadesk`), and `bundle.publisher` / `bundle.copyright` are set to
  `dirttw`, so the installer's `Manufacturer` and the exe's `CompanyName` read
  `dirttw` (Tauri would otherwise derive `github` from the identifier's second
  segment) and the MSI `UpgradeCode` is regenerated from the real id. The app
  data moves to `%APPDATA%\io.github.dirttw.wadesk` (Linux:
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

<!-- Version comparison links. The earlier pre-release tags were removed, so the
     diffs are anchored on their release commits. -->
[0.1.0-rc.6]: https://github.com/dirtTW/wa-desk/compare/v0.1.0-rc.5...v0.1.0-rc.6
[0.1.0-rc.5]: https://github.com/dirtTW/wa-desk/compare/v0.1.0-rc.4...v0.1.0-rc.5
[0.1.0-rc.4]: https://github.com/dirtTW/wa-desk/compare/v0.1.0-rc.3...v0.1.0-rc.4
[0.1.0-rc.3]: https://github.com/dirtTW/wa-desk/compare/68a19c0...v0.1.0-rc.3
[0.1.0-rc.2]: https://github.com/dirtTW/wa-desk/compare/ab8027f...68a19c0
[0.1.0-rc.1]: https://github.com/dirtTW/wa-desk/compare/83c1dc2...ab8027f
