# WA Desk — development notes

This file keeps the feature inventory, architecture notes, design gotchas and roadmap that used
to live in `README.md`; the change log is its own file, [`CHANGELOG.md`](CHANGELOG.md). Build
instructions and dependency/license information stay in `README.md`.

## Change log

See [`CHANGELOG.md`](CHANGELOG.md) — newest first, one entry per feature or
fix, with the mechanism and the verification signal.

## What works today

| Area | Status |
| --- | --- |
| QR linking | ✅ |
| Phone-number pairing code | ✅ |
| Persistent session (SQLite device store) | ✅ |
| Chat list from WhatsApp history sync | ✅ names, avatars, timestamps, unread counts |
| New chat picker | ✅ the + button opens WhatsApp-style search over saved contacts, known chats and raw numbers |
| Chat history from WhatsApp history sync | ✅ text, media captions, own/other bubbles |
| Photo &amp; video playback | ✅ images inline, videos open in the viewer, expired media recovered via media-reupload |
| Media viewer | ✅ full-screen image/video viewer with filmstrip, video posters, custom player, prev/next, arrow keys, scroll zoom, drag pan, auto-hiding chrome when zoomed |
| Save media | ✅ the viewer's download button copies the file to `Downloads\WA Desk` instead of navigating the WebView |
| In-chat search | ✅ header search with match counter, previous/next, yellow highlight and Esc to close |
| Voice / video calls | ✅ 1:1 call signaling through `whatsapp-rust`'s VoIP runtime; microphone/audio via Web Audio, H.264 video via WebCodecs; needs a microphone/camera |
| Poisoned sender recovery | ✅ rate-limited reconnect + one retry after a transport failure poisons the Noise sender |
| Channels panel | ✅ followed channels have their own panel with real names from the newsletter metadata; posts open read-only |
| Status / stories | ✅ recent/viewed lists with green/gray rings, right-side “Share statuses” pane, old statuses recovered from `status@broadcast` history, 24 h story viewer with progress bars, cross-contact next/prev, reply/reactions, text/image/video posting, delete your own |
| Chat export import | ✅ load a WhatsApp `.txt` export into the open chat when the phone cannot backfill messages sent while it was offline |
| Mentions | ✅ `@mentions` render the contact or own display name (LID-only mentions included), with the raw id only as a fallback |
| Chat history reload | ✅ opening a chat reloads it through the client's own linked session (`HistorySyncOnDemand`), anchored on the oldest message like WhatsApp Web; scrolling pages further back, and a fresh window re-syncs from the disk chat cache |
| Self-chat name | ✅ the account's own chat is labelled `(You)` in the list and header |
| Contact profile photos | ✅ fetched lazily, cached, initials as fallback |
| Send / receive text | ✅ |
| Receive media messages (caption + type shown) | ✅ |
| Audio & voice-note playback | ✅ official-style player bubble: play/pause, seek + drag, duration/countdown from the wire (`audioMessage.seconds`, waveform for `ptt`), orange headphones chip |
| Typing indicators, read receipts | ✅ |
| Auto-reconnect with backoff | ✅ (library) |
| Local cache so the list survives restarts | ✅ (WebView storage) |
| Saved contact names from the phone address book | ✅ full `contact` app-state sync on first launch, persisted locally |
| New contact | ✅ name + international number written as a `contact` app-state mutation, then opened as a chat |
| AI assistant | ✅ bring-your-own OpenAI-compatible key (encrypted at rest), on/off toggle, `/ai` requirement toggle, casual Indonesian "gw/lu" persona, replies to text/photos/stickers/GIFs/short videos, occasional reactions/random stickers |
| AI chat context | ✅ last ~24 messages from the chat (groups and DMs), capped at ~4,000 characters and twelve hours |
| Link previews | ✅ WhatsApp's embedded preview (title, description, thumbnail); Open Graph fallback when scraping is allowed |
| Group bubbles | ✅ the sender's profile photo in the left gutter and their name (coloured per sender) once per sender run, like official |
| Replies / quotes | ✅ quoted message box with author, caption and photo thumbnail; the pending reply shows an inset preview card above the composer with the sender's colour; @mentions render green; the AI reads the quoted text/photo |
| Bulk photos / videos | ✅ consecutive media collapses into an album grid |
| Paste an image from the clipboard (Ctrl+V) | ✅ draft preview; keep pasting to stack up to 10 images, composer text becomes the caption |
| Deleted chats / messages | ✅ kept locally; the eye button in the left rail reveals them |
| Message edits and revokes | ✅ applied live (`MESSAGE_EDIT` / `REVOKE` protocol messages) |
| Long-press a bubble | ✅ Edit / Delete for me / Delete for everyone |
| Media download/upload, reactions, group management | ✅ media, emoji reactions, create group, info, add/remove/promote/demote members, leave group |
| Sticker collection | ✅ grid of up to 80 cached stickers from chats, with count; the AI can use the same collection |
| Favorite stickers | ✅ fetched from the linked session's `favoriteSticker` app-state, downloaded as WebP; tap sends with an optimistic bubble and closes the picker immediately |
| Sticker panel | ✅ WhatsApp-Web-style recent/favorites/packs tabs, sticker-store search field, Create tile, and Emoji/GIF/Stickers footer |
| Sidebar menu | ✅ the header kebab: New group, Starred messages, Select chats, Mark all as read, App lock (honest "not supported" toast), Log out |
| Starred messages | ✅ star/unstar from the message menu through the `star` app-state collection; the Starred messages screen lists them and opens the chat on the message |
| System tray, notifications, autostart | ✅ tray menu (Open / Quit), desktop notifications per message, launch at login |

WhatsApp only uploads history **when a device is newly linked**. If the client was paired
before this feature existed, the list stays empty until the device is unlinked and paired
again. Live traffic always works.

---

## Layout

```
Cargo.toml                     workspace
crates/wa-types/               serializable UI vocabulary (serde only)
crates/wa-ui/                  UI model: strings, plans, timings, HTML renderers
  src/lib.rs                   bootstrap payload, re-exports, integration tests
  src/text.rs                  WhatsApp markup, mentions, previews, ticks, phones
  src/labels.rs                clock/stamp/day labels, follower counts
  src/html.rs                  escaping, Material icons, avatars
  src/emoji.rs                 emoji catalogue search + picker markup
  src/emoji_data.rs            @generated catalogue (2,300 emoji, 9 groups)
  src/icons_data.rs            @generated Material Symbols paths
  src/chat.rs                  message list: bubbles, media, quotes, reactions, ticks
  src/composer.rs              status composer palette, fonts, ARGB, emoji panel
  src/state.rs                 merge/trim plans for the chat store, photo queue
  src/cache.rs                 cache fingerprint, payload, restore, photo queue
  src/actions.rs               status permissions/drafts, reply banner, selection bar
  src/chat_actions.rs          chat-menu/bulk/message/star/mute/member plans, delete targets, info
  src/context.rs               AI context/stickers, media limits, poster sampling
  src/calls.rs                 call overlay, guard, codec/audio formats, jitter buffer
  src/search.rs                in-chat search matching, hit index, counter label
  src/incoming.rs              message-event folding, unread/receipt rules, receipt batches
  src/reactions.rs             reaction toggle, live-event fold, own-pill replace
  src/export.rs                chat-export parsing and import merge plan
  src/history.rs               history anchors, re-sync order, gate and progress
  src/screens.rs               channels panel, community screen, starred messages
  src/status.rs                status rail list, story viewer shell, views list,
                               viewer chrome, step targets, view receipts, healing
  src/picker.rs                new-chat / forward picker rows and sections
  src/settings.rs              every Settings page, update row, AI form, credits
  src/menus.rs                 chat/message menu rows, mute submenu, delete dialog
  src/media.rs                 media hydration plan, download guard, evictions
  src/scroll.rs                scroll metrics, event plan, visible anchor, disc offset
  src/viewer.rs                media viewer chrome, video player, filmstrip
  src/stickers.rs              sticker picker grid, cells and empty states
  src/sidebar.rs               chat list: filters, pin order, communities, search, rail badges
crates/wa-core/                protocol session layer (no UI dependency)
  src/lib.rs                   Session struct + module wiring
  src/events.rs                protocol event -> UiEvent translation
  src/session.rs               lifecycle, identity, chat resolution
  src/calls.rs                 VoIP call state and media channels
  src/contacts.rs              contacts, profiles, peer lookup
  src/groups.rs                group create/info/member actions
  src/history.rs               on-demand history and chat cache
  src/media.rs                 upload/download, stickers, favorites
  src/messaging.rs             text, edits, reactions, receipts
  src/status.rs                stories: history, post, reply, react
  src/util.rs                  parsing/media-cache helpers
  src/tests.rs                 unit tests
apps/desktop-native/         NATIVE front end (eframe/egui, no WebView, no JS)
  src/main.rs                runtime, session wiring, --selftest entry
  src/model.rs               chat store from UiEvents, search, unread, labels
  src/login.rs               QR / pairing-code gate state (pure fold)
  (media: Model::media + egui textures, one fetch per message)
  (status: Model::statuses + viewer via wa_ui::status)
  (channels: Model::channels + read-only post list)
  src/settings.rs            native settings: theme + AI form (key never on disk)
  src/app.rs                 egui window: chat list + conversation + composer
vendor/whatsapp-rust/          patched protocol client (crates.io 0.7, [patch.crates-io])
vendor/wacore-appstate/        patched app-state crate: snapshot MAC recovery (see change log);
                               excluded from the workspace (its `simd` feature needs nightly),
                               tested standalone: `cargo test --no-default-features`
```

The dependency direction is one-way: `wa-core` knows nothing about the UI, and
`wa-ui` knows nothing about the session, so either can be reused alone.

### How the pieces talk

```
WhatsApp servers
      │  noise + Signal (whatsapp-rust)
      ▼
   wa-core::Session  ──UiEvent──▶  std mpsc  ──▶  wa-desk-native (egui window)
      ▲                                                      │
      └────────────  Command (send_text, mark_read, …)  ◀─────┘```

Protocol payloads never cross the IPC boundary: `wa-core` reduces them to
`MessageView` / `UiEvent`, which serialize as `{ "type": "message", ... }`.

### The UI model lives in Rust (`wa-ui`)

The window is a painter; every decision is a function in `wa-ui` that returns
strings, plans or numbers:

```
chat row      -> wa-ui::labels::stamp_of + wa-ui::text::preview_of
bubble ticks  -> wa-ui::text (read/delivered labels)
status viewer -> wa-ui::status (groups, chrome, progress)
group screen  -> wa-ui::labels::group_member_count
settings      -> wa-ui::settings (theme, AI form plan)
```

- `crates/wa-ui` has no UI-framework or protocol dependency: it is pure serde
  in / values out, and every module is unit-tested (`cargo test -p wa-ui`).
- The data tables are checked-in Rust. `icons_data.rs` regenerates from
  `scripts/material-paths.json` with `cargo run -p wa-ui --example gen_icons`
  (then `cargo fmt`); `emoji_data.rs` is a one-off generated catalogue whose
  upstream dataset is no longer in the tree, so it is hand-edited if it ever
  needs to change. **There are no Node, `.mjs`, `.ps1` or `.bat` files left in
  the repository** - the icon generator is the Rust example above.
- `wa-ui` still carries the HTML renderers; they are the shared source of truth
  for markup decisions and their tests. The native window does not consume the
  HTML — it paints with `egui`.

---

## Where data lives

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\io.github.dirttw.wadesk\whatsapp.db` |
| Linux | `~/.local/share/io.github.dirttw.wadesk/whatsapp.db` |

Delete that file to unlink locally (and any other device will still show the linked
device until you remove it from the phone).


## Design notes / gotchas

- **The shipped app is `apps/desktop-native` (`wa-desk-native`).** The WebView
  front end and its 13 JS files were removed in rc.57; `wa-ui` remains the
  shared decision layer and the native window paints with `egui`.
  `cargo run -p wa-desktop-native -- --selftest` folds a scripted event
  sequence headlessly (gate, chats, media, status, channels, settings, group
  info) and prints what the window would show, so the UI is verified in CI
  without a display.

### Search, pickers and the media viewer (v144)

- **In-chat search** lives in the header: `wa_ui::search::matched_indices` decides which
  bubbles match, `count_label` writes the `2/5` counter and `hit_index` wraps the stepper.
  The window paints the query's ranges on a yellow band inside the matched bubble with a
  `LayoutJob` (char-aligned lowercase matching, so Unicode does not shift offsets) and
  outlines the bubble; Esc closes the bar and the state resets when another chat opens.
- **Emoji picker** reads the same catalogue as the old shell (`wa_ui::emoji::groups` /
  `search`), keeps recents through `remember_recent`, and inserts with `insert_text` so the
  composer and the stored key cannot disagree about where a glyph landed.
- **Sticker picker** lists the account's favourites (fetched once through
  `Command::FavoriteStickers`, sent with `send_favorite_sticker`) above the stickers already
  cached from chats (`forward_sticker`). A sticker bubble paints without a bubble, 128px wide.
- **Media viewer** pages the open chat's photos and videos (`Model::media_items`), with a
  filmstrip, arrow keys, Esc, and Save copying the cached file into `Downloads\WA Desk` in
  Rust - the window never opens a second process.

### The chat list (v147)

- The sidebar is three pieces: a 58 px **rail** (`rail_button`) with unread/unseen badges and the
  own avatar, the **Chats** panel (kebab + green FAB, rounded search, filter pills) and the rows.
  `Model::filter_counts()` drives `All / Groups N / Communities N / Unread N`; the pill filters
  `visible_chats`, and `total_unread` feeds the rail badge.
- The own avatar needs `Model::me_jid`: `UiEvent::SelfIdentity` is emitted once the session
  starts (`own_jid()` or the LID, plus `push_name()`), so `contact_photo` has a real JID.
- History arrives in batches: `InitialSync` seeds the address book (bare numbers normalised to
  `@s.whatsapp.net`) and asks for eight chats; the window sends `SyncMore` every eight seconds
  for six rounds so the recent chats and groups surface like the phone's list. Opening a chat
  with no history asks for that chat alone.
- `--preview` never writes `chats.json`, so the demo rows cannot pollute a real account.

### The native look (v146)

- `apps/desktop-native/src/theme.rs` holds the official `--WDS-*` tokens as a `Palette`
  (panel / rail / wallpaper / elevated / hover / selected / text / secondary / accent /
  bubble in-out / link / read / danger) and `apply()` installs them as egui `Visuals` plus the
  WhatsApp type scale, menu rounding and shadow. The theme choice is applied every frame, so
  Light / Dark / System really switches the window.
- **Bubbles are measured, not framed.** A `Frame` let a right-aligned time label stretch the
  bubble to the full width and height (the first bubble filled the screen); the window now
  lays the body out with a `LayoutJob`, allocates exactly that size, and paints the rounded
  rect, sender name, body and clock itself. The tail is one square corner (`CornerRadius`).
- **Short chats sit at the bottom.** The scroll content is padded by the previous frame's
  measured height (`ctx.data`), so a chat with three messages hugs the composer.
- **Icons come from the Material Symbols set** (`wa_ui::icons_data`, the same table the old
  shell used). egui cannot draw SVG paths, so `icons.rs` rasterises each one with `resvg` at
  the requested size and tint, caches the texture on the context keyed by name/size/colour, and
  `draw_icon` paints it; `material_icons_rasterise_into_visible_pixels` guards the pipeline.
  Never font glyphs - egui's bundled fonts lack most emoji and render boxes.
- **The menu** is the WhatsApp card: quick reactions on top, then icon + label rows with a
  rounded hover highlight; the rows still come from `wa_ui::menus::message_menu`.
- **Visual verification without a session**: `--preview` loads `Model::demo()` and
  `--screenshot <path>` saves one settled frame with `ViewportCommand::Screenshot` and exits.
  That is how the release screenshots were produced.

### Message actions (v145)

- Right-click is the whole entry point: `bubble()` returns a `BubbleAction` and the
  conversation runs it, so painting never sends a command itself. The menu rows are
  `wa_ui::menus::message_menu` flags (edit only your own text, reply/copy/forward/star off
  for a gone message), the delete question is `wa_ui::menus::delete_confirm` (for-everyone
  only when the message is ours and the chat is not "Message yourself"), and the reply card
  is `wa_ui::actions::reply_preview`.
- Reply/edit share the composer: `Model::send_label()` relabels the button, `editing` loads
  the body in, and cancelling drops an abandoned edit's draft. Sends go through
  `send_text_reply` / `edit_message`; `MessageEdited` rewrites the stored bubble.
- Reactions are optimistic (`apply_own_reaction`) and then folded from `Reaction` events;
  ours replaces our previous emoji rather than stacking, and an empty emoji removes it.
  Pills render under the bubble, ours in the read-receipt blue.
- Forward is a searchable multi-select picker (`forward_rows`), one `forward_message` call
  for every chosen chat.

### Official-client parity pass (v140)

- Clock format is globally `8:44 pm` — lowercase 12-hour with colon, forced
  `en-US`, independent of the OS locale (`20.47` came from id-ID). Applies to
  bubbles, stamps and the viewer.
- Chat header now carries exactly the official two icons — search and the ⋮
  kebab (`#chat-import`, `#chat-voice`, `#chat-video` are CSS-hidden). The
  kebab opens the official-style dropdown: Contact info / Group info, Voice
  call, Video call, Import chat export — so calls and export/import stay
  reachable while the header matches 1:1.
- Sidebar filter row matches the official pill shape: All, Unread,
  Favourites (inert, "not supported yet" tooltip), Communities (the former
  Groups filter) and the "more filters" caret.
- Material 3 typography: the shell sets an explicit font stack
  (`Segoe UI Variable Text / Segoe UI / Roboto`, M3 default fallbacks) so the
  app never drifts onto the WebView default face.
- Self-chat ticks: own messages in "Message yourself" land straight on the
  grey `✓✓` (the phone owns the chat — `isSelfJid` helper + the addSent hook
  set `status: "delivered"` and `tickHtml` short-circuits self-chat), matching
  the official grey double tick before any receipt. Verified on the live dev
  instance: 4× `✓✓|grey`, renderer font reports `Segoe UI`, header shows only
  `chat-search` +
  `chat-menu` visible with import/calls `hidden`, and the kebab reads
  `Contact info / Voice call / Video call / Import chat export`.
- **Rendered side-by-side verified (v141+)**: full-window PNG capture of the
  v141 self chat (`Page.captureScreenshot` → AppData/Temp) against the
  official client: RGBA message bubbles — `#005c4b` outgoing with 7.5px
  radius, 2px gap inside a sender run / 6px before a new "first" bubble,
  caption "↱ Forwarded" above the media, mention green `#06cf95`, blue links
  `#53bdeb`, reaction pill, grey `✓✓` / blue read ticks and `8:44 pm` stamps
  all line up. The sidebar header now reads **"Chats"** with the compose
  button, ⋮ menu and the theme toggle instead of a "WhatsApp" title, and the
  post-v141 `build.rs` rerun-walk keeps release builds current with UI edits
  (a rebuild with the "Chats" header ships `sideTitle: "Chats"`,
  `sideMenu: true` on the live DOM capture).
- **Rail aligned to the official order (v142)**: Chats → Calls → Status →
  Channels → (deleted-history eye, kept for the local archive) → Settings →
  profile; the new rail-calls button starts a voice call in the open chat
  (self chat/groups get the official hint to use the contact/group screens).
  CDP capture lists the live order and the light theme — the light palette is
  the official WhatsApp Web light tokens (`--bg #f5f1eb`, `--panel #ffffff`,
  `--rail #f7f5f3`, `--bubble-out #d9fdd3`, `--bubble-in #ffffff`); a light
  screenshot (`v142-wadesk-light.png`) plus the dark one serve as the
  side-by-side evidence.
- **Material You 3 respect**: the app's tonal surfaces map to the official
  M3 roles (background/surface/surface-container/primary-container/
  secondary-container), the shape scale uses M3 rounding (999px pills,
  7.5px bubbles, 48px icon discs), the M3 typography face stack is set, and
  Material Web components (M3 `md-*`) power the settings buttons. A full M3
  token re-architecture (runtime scheme switch, elevation levels) stays a
  follow-up — the current styling is the official client's visual language
  1:1 rather than the Google sample theme, which is exactly what "same ui
  like the offcial one" asks for.
- **The palette is the official design-token set, not a hand-tuned guess.**
  `styles.css`'s `:root` blocks carry the values of WhatsApp Web's `--WDS-*`
  tokens, read live from `web.whatsapp.com` (the browser MCP's `eval` on the
  real page; the stylesheet is
  `static.whatsapp.net/rsrc.php/v5/y7/l/0,cross/…css`; the one-off extraction
  scripts that read the dark and light theme blocks offline were removed with
  the WebView shell). Dark: surface `#161717`, rail `#1D1F1F`,
  elevated `#242626`, hover/selected `rgba(255,255,255,.1)`, text `#FAFAFA`,
  secondary `rgba(255,255,255,.6)`, accent `#21C063`, pill selected `#103529`,
  outgoing bubble `#144D37`, link `#21C063`, read `#53BDEB`, danger `#FB5061`.
  Light: surface `#FFFFFF`, rail `#F7F5F3`, wallpaper `#F5F1EB`, accent
  `#1DAA61`, outgoing bubble `#D9FDD3`, link `#1B8755`, read `#007BFC`.
  Sender-name colours and default avatars use the official
  `--WDS-components-profile-photo-content-*` / `-surface-*` palettes.
  Verified at the time by capturing both windows and sampling the same elements
  (the capture/sample scripts were removed with the WebView shell; the capture
  path was trustworthy because it reproduced our own known CSS values exactly):
  panel `#161717`, rail `#1D1F1F`, chat `#161717`, header `#161717`, search
  `#2E2F2F`, selected row `#2E2F2F`, incoming bubble `#242626`, FAB `#21C063`
  — identical in both clients.
  Secondary screens: every screen container is `#161717`, every menu/popover is
  `#242626`, the media-viewer backdrop is `#161717`, the status-viewer stage is
  `#0B141A` (the official `--status-background-rgb`), and the status composer
  palette is the official 20-colour array. The former `theme-audit.mjs` check
  compared every colour literal in the UI against the 1,222 values parsed from
  the official stylesheet; it went with the WebView shell, and the native
  colours are the tokens listed above.
- Verified on the live dev instance via CDP: `shownTime:
  "10:02 am✓"`, kebab items `[info, voice, video, import]`,
  `callBtnHidden: true`, pills `All/Unread/Favourites/Communities/caret`.

- **Rendering is budgeted.** The sidebar and the chat are full rebuilds (`innerHTML`), so redundant
  work is what costs: `scheduleHistoryRender()` coalesces history chunks, `scheduleChatRender()`
  coalesces document/thumbnail hydration, `scheduleChatListRender()` coalesces sidebar bursts, and
  search input is debounced. Date labels are memoized (`TIME_LABELS`/`STAMP_LABELS`/`DAY_LABELS`)
  and photo replies patch a single avatar. When adding a new render call, prefer the scheduled
  variant unless the caller reads the DOM synchronously afterwards.
- **Nothing Tauri is left.** `tauri`, `wry`, `webview2-com`, `webkit2gtk` and the
  `tauri-plugin-*` crates are gone from `Cargo.toml`/`Cargo.lock`; the window is
  `eframe`/`egui`, QR codes are rasterised with `resvg`, and photos decoded with
  `image`.
- **The UI is compiled in.** `wa-desk-native` links `wa-ui` directly; there is
  nothing to embed, reload or cache, so a Rust change is the only way to change
  the interface.
- **UI events are at-least-once.** `Event::Messages` can replay on redelivery, so the
  frontend deduplicates by message id.
- **History replay.** `Event::HistorySync` chunks carry conversations *and* their messages.
  History bodies are not Signal-encrypted per message — they are protected by the
  encrypted transport — so `wa-core` decodes them straight from the protobuf and emits
  `UiEvent::HistoryMessage`. That is the same source WhatsApp Web renders when you link a
  fresh device. Push names and PN↔LID aliases can arrive in a *later* chunk than the
  conversations, so both are accumulated and late names are re-emitted as `ChatName`.
- **Saved contact names** come from the `contact` app-state collection (`Event::ContactUpdate`).
  Linked devices only receive deltas after pairing, so the first launch asks for a full
  snapshot; the local app-state version is rewound first, otherwise the server's snapshot
  is older than the local cursor and the sync fails with a version mismatch. Names are
  persisted to `contacts.json` and win over push names everywhere, including group senders.
- **New contact** writes a `contact` app-state mutation through
  `client.chat_actions().save_contact`, updates the local `contacts.json`, emits the contact-name
  event and opens the new conversation.
- **AI assistant.** Bring your own OpenAI-compatible endpoint (the default is OpenCode Go,
  `https://opencode.ai/zen/go/v1`). The API key is encrypted with AES-256-GCM using a random
  `ai.key`; only a `••••last4` hint is ever returned to the webview. Auto-reply runs in Rust on
  live messages from **saved contacts** (DMs) and on **group messages that explicitly @mention
  you** (never yourself, never history replays). It keeps a short per-chat history, rate-limits to
  one reply every 2 s and shows a typing indicator. Replies wait a human-like delay (roughly
  1.2–3.2 s plus 160 ms per character, capped at 7 s) and a 5 s global gap between AI sends, so
  bursty chats do not look like spam. The default system prompt copies a casual
  Indonesian texting style (**“gw”/“lu”**, short 1–2 line replies, lowercase, almost no emoji,
  occasional “wkwk”/“yaudah” but not every message), stays in character, and refuses attempts to
  change its name or leak the system prompt. The bot uses the linked account's display name as its
  own name, so it answers as the account owner. Replies are structured
  JSON internally: the model may also pick one
  reaction emoji (at most once per chat every 3 minutes). Stickers are chosen by a separate
  vision pass: the client shows the model several candidate sticker images plus the recent chat,
  and it picks one that fits (normally at most once every 3 minutes). If someone explicitly asks
  for a sticker ("kirim stiker"), selection is forced with up to 12 candidates, the sticker
  cooldown drops to 20 seconds, and the text reply is sent faster so the sticker follows quickly.
  Emojis inside reply text are limited to roughly one reply every 2 minutes; otherwise they are
  stripped. Incoming stickers get a short text
  reply (the AI may also answer with a sticker). Incoming photos are sent to the optional
  vision model; leave it empty to use the chat model (the default OpenCode Go model accepts images).
  For short videos and WhatsApp GIFs, the client extracts up to three frames with `ffmpeg` (from
  `WA_DESK_FFMPEG`, next to the exe, or on `PATH`) and sends them as images; without ffmpeg it falls
  back to the video's embedded JPEG thumbnail. If media input fails, the reply falls back to the
  caption/text path. In groups, an @mention or a
  reply to one of your messages is enough while AI mode is on, including a mention attached to a
  photo (the mentioned photo is read with vision and answered). `/ai ...` still triggers a reply
  when AI mode is off: in a saved DM, in "Message yourself", or in a group when the message also
  mentions you (for example `/ai @alice summarise this`). When someone replies to a message, the
  quoted text is included in the prompt, and a quoted photo is attached for vision when the reply
  itself has no photo. The model also receives a bounded transcript of the last ~24 messages
  from that chat (both groups and DMs), capped at ~4,000 characters and twelve hours, so it follows
  the topic without forwarding the whole conversation. With
  **Require /ai trigger** off and AI mode on, every saved-contact message gets an instant
  reply. Unsaved contacts are never answered. OpenCode Go requests include the required
  `x-opencode-session` header.
- **Link previews** are not scraped on the receive path. WhatsApp embeds the preview
  (`title`, `description`, `jpegThumbnail`) in `extendedTextMessage`, so `wa-core` forwards
  those bytes and the webview renders the same card WhatsApp Web shows. Open Graph scraping
  (`link_preview` command) is only a fallback for links this client sends or older messages
  that carry no embedded metadata.
- **Reactions** arrive as `reactionMessage` annotations (not bubbles) and are stored on the
  target message, grouped by emoji. Sending one uses `client.send_reaction` with the target
  key; changing a reaction revokes the previous emoji first so the server replaces it, and
  events carry `senderTimestampMs` so a delayed echo cannot revert a newer change. The quick
  bar has six emojis plus `+` to pick any emoji from the full picker.
- **Media viewer.** Clicking an image (including any cell of an album) opens a full-screen
  viewer; videos open there too. Inline videos show a poster frame plus a duration badge, and
  the viewer has a custom transport bar (play/pause, seek, time, 1×/1.5×/2×/0.5× speed, mute,
  fullscreen) instead of native controls. The strip lists the chat's photos/videos, arrows/←/→
  step through them and Esc or the backdrop closes it. The download button copies the file to
  `Downloads\WA Desk` through Rust (no browser navigation involved).
- **In-chat search.** The header search button opens a search bar with a match counter,
  previous/next buttons, yellow highlighting and Esc to close. It searches text and media
  captions in the open chat, not the chat list.
- **Connection recovery.** If a transport failure poisons the Noise sender (WhatsApp
  refuses to reuse a possibly-replayed write keystream), the client waits for the reconnect
  cooldown, reconnects immediately, waits for full readiness, then retries the failed send
  exactly once. Reconnects are rate-limited to one per 8 seconds and are never looped, so a
  flaky network cannot turn into a reconnect/retry storm. A recovered transport does not always
  re-emit `connected`, so while the webview believes it is offline it reconciles the badge with
  `get_status` every two seconds; a healthy socket clears the banner instead of leaving a
  permanent “reconnecting” state.
- **Chat history reload.** Opening a chat sends WhatsApp's `HistorySyncOnDemand` request through
  the client's own linked session and merges the chunk that comes back. Like WhatsApp Web, the
  request is anchored on the **oldest** stored message, so every request pages further back;
  scrolling to the top repeats it. A chat that history sync only announced has no anchor, and the
  phone answers with that chat's latest window. Duplicates are dropped by id and new messages are
  inserted in timestamp order. Chats with disappearing messages (24 h) cannot return bodies older
  than that window, matching WhatsApp Web.
- **Status history.** Opening Status asks the linked session for `status@broadcast` history. Those
  messages are converted into the same `StatusUpdate` events as live stories, so updates received
  while the app was closed appear in Recent/Viewed. Status receipts carry the author as the
  participant, which the server needs to map the ack back to the poster.
- **Channels panel.** WhatsApp channels (`@newsletter`) are kept out of the chat list and shown
  in their own rail panel: followed channels are listed with their real names from the newsletter
  metadata, and opening one shows the posts read-only (no composer or calls). New channel
  broadcasts arrive through the normal message path, so they appear in the panel and the post
  view without extra bookkeeping.
- **Chat export import.** On-demand history only pages *behind* the oldest local message, so
  messages sent while the primary phone was offline cannot be requested back from it. The
  download button in a chat header loads a WhatsApp “Export chat” `.txt` file into that chat:
  timestamps, senders and multi-line bodies are parsed, messages already present are skipped, and
  the imported history is saved with the rest of the session. Re-export from WhatsApp Web for the
  newest window if it is newer than the last export.
- **Mentions.** Group mentions arrive as LID or PN user parts. The renderer resolves the account's
  own JID/LID first (using the self-chat handle, e.g. `@alice` → “Alice”), then maps
  other LIDs through the resolved phone/contact tables, and finally falls back to a sender name
  already known in the chat. A raw id is shown only when no name is known.
- **Restart / fresh-open recovery.** The chat list is mirrored to `chats.json` next to the
  contact store, so a restart (or a cleared WebView cache) still has a sidebar. When no chat has
  any message, the client seeds personal chats from the address book and re-requests history for
  a small batch (max 8 chats, 3 s apart, once per 24 h). Settings → Chats has a manual
  **Sync chat history** action with the same pacing. Every history request is serialised with a
  2 s quiet gap and a 2-minute per-chat cooldown, so switching chats quickly cannot turn into a
  burst of peer messages.
- **Business chats.** WhatsApp stores a business account under both a LID and a phone
  number, and sends its content as templates rather than plain text. The client looks the
  peer up through `usync` (`resolve_peers`), folds the duplicate LID row into the phone-number
  chat, and shows the verified business name (e.g. `Example Store`). On-demand history replies are
  matched back to the row the user opened, so clicking a LID business chat no longer creates a
  second conversation. Hydrated and interactive templates (text, header image/video/document)
  are decoded for both live messages and history backfill.
- **Self chat label.** The account's own conversation is shown as `name (You)` in the
  chat list and header, matching WhatsApp Web.
- **Voice/video calls** are not in the native window yet; the VoIP runtime still
  lives in `wa-core` and waits for a native audio/video path.
- **Light/dark theme.** Settings → Chats → Theme (or the moon/sun button in the sidebar
  header) switches between Light, Dark and System default. Both palettes are the official
  `--WDS-*` design tokens (see the palette note above), not hand-tuned values. The dialog and
  its radio buttons are Material Web components; the shell keeps the WhatsApp Web layout. The
  components are bundled locally in `ui/vendor/material-web.js`, so the client stays offline
  and does not load anything from a CDN.
- **Groups** use the library's `Groups` feature: create, read metadata for the info screen,
  add/remove participants, promote/demote admins, and leave. The `+` menu opens the creation
  dialog with a contact checklist; the same dialog doubles as "Add members".
- **Notifications, tray and autostart** have to be rebuilt natively (the tray and
  notification plugins went with the removed WebView shell); autostart still works
  through the OS startup folder.
- **Media is downloaded on demand.** The Rust side keeps the protobuf of recent
  media messages, downloads/decrypts on request and writes the plaintext into
  `<app data>/media`. If the CDN path has expired (always the case for photos
  replayed from history), it asks the server to re-upload via
  `client.media_reupload()` and retries once — the same flow WhatsApp Web uses.
- **Profile pictures** are fetched a few at a time for the visible chats
  (`contacts().get_profile_picture`, which also answers for `@g.us`), and rendered straight from
  `pps.whatsapp.net`. A chat with no picture keeps the WhatsApp empty state: the two-person glyph
  for a group, coloured initials for a person. Requests made while the socket was down are
  retried when the connection comes up (and when the app starts already connected), and a URL that
  fails to load is evicted and retried after 2 minutes.
- **Memory is bounded.** The UI keeps at most 250 messages per chat and 20k across
  the session (oldest conversations are trimmed first, the open chat is protected);
  downloaded media URLs are an LRU of 400. `wa-core` keeps 1.5k media protobufs in
  memory and prunes `msgcache`/`media` to the newest 3k/4k files on startup.
- **Stickers** render without a bubble. The picker follows WhatsApp Web: recent/favorites/packs
  tabs, a sticker-store search field, a Create tile, and an Emoji/GIF/Stickers footer. Tapping a
  sticker (favorites included) adds the optimistic bubble, closes the picker immediately, then
  swaps in the server id; this prevents a slow upload from turning one tap into several sends.
  `.webp` files can be added with their real canvas size parsed from the WebP header.
- **Emoji** uses the full Unicode set (2,300 glyphs in 9 groups) with search and
  recents, and emoji-only messages render large like WhatsApp Web.
- **Own avatar** is resolved from the paired PN/LID. The photo may be cached under either id, so
  the rail prefers whichever has an image and refreshes when the self chat, `paired`, or photo
  fetch arrives; it therefore does not stay on “?” after a fresh pair.
- **No JID parsing in the UI.** `wa-core` remembers every chat JID it has seen and
  resolves by string, which avoids re-parsing and rejects unknown chats.
- **Reconnect is the library's job.** `Disconnected` is informational; auto-reconnect
  uses Fibonacci backoff. `LoggedOut` / `StreamReplaced` are terminal — the account must
  be linked again.
- **Send failures are silent on disk.** A `SendResult` means the stanza was accepted,
  not delivered. Delivery/read state needs receipt handling in the UI.
- **Linux** builds against `eframe`/`glow`, so X11 or Wayland plus a GL driver is
  all it needs; there is no WebKit dependency to fight with.
- **`whatsapp-rust`'s `simd` feature is off on purpose.** It enables
  `#![feature(portable_simd)]`, which only compiles on nightly. Everything else uses
  the crate's default feature set, so stable Rust works. If you want the extra
  throughput, build with `RUSTC_BOOTSTRAP=1` or a nightly toolchain and re-enable it.

- **In-app updates are native now** (`apps/desktop-native/src/updates.rs`). Every WA Desk
  release is a **pre-release**, so GitHub's `/releases/latest` 404s - the updater lists
  `/releases?per_page=20`, ignores drafts, keeps the highest tag that is newer than
  `CARGO_PKG_VERSION` and ships this platform's asset (`wa-desk-native.exe` / `wa-desk-native`),
  then downloads it with `ureq`. Version order is the numeric core first - so a bigger core
  always updates (`0.2.0-rc.60` → `0.3.0-rc.1`, → `1.0.0`) - then a final release above any
  pre-release of the same core, then the `rc.N` number compared as a number (`rc.10` beats
  `rc.9`). Installing renames the running exe to `.old`, moves the download into place, spawns
  it with `--restarted` and exits; the new process sleeps
  1.5 s so the old one releases the database and the instance mutex. The check runs once on
  launch and on demand from Settings → Updates; `--check-updates` / `--download-update` verify
  the network path from the CLI (`WA_DESK_UPDATE_FROM` overrides the installed version).
- **The security posture is audited in [`SECURITY.md`](SECURITY.md).** CI runs
  `cargo audit`; accepted advisories live with their rationale in
  `.cargo/audit.toml`. `security.csp` is set (it used to be `null`) — if you add
  a remote resource, check the CSP allows it, and prefer moving an inline
  `on*` handler to `addEventListener` over loosening `script-src`.

- **Windows installer** (`apps/windows-installer`). The setup exe is the stub binary with the
  app appended and a 16-byte trailer (`WADSETUP` + payload length); `pack-setup` writes it and
  refuses to ship unless the payload round-trips. The stub copies the app to
  `%LOCALAPPDATA%\Programs\WA Desk`, writes `Uninstall WA Desk.cmd`, creates Start Menu and
  Desktop shortcuts via `IShellLink`/`IPersistFile` COM, and launches it. It lives in its own
  crate so the app binary never links the `windows` crate.
- **CI** keys its `actions/cache` on the Rust version rather than `Cargo.lock` (a version bump
  used to invalidate the whole dependency tree), disables incremental and dev debug info,
  installs `cargo-audit` from a prebuilt binary, and publishes `WA-Desk-Setup.exe` beside the
  raw binaries.

- **Restart cache and history recovery.** `wa-core` only receives history for a *newly linked*
  device, so `apps/desktop-native` mirrors the chat list to `chats.json` (names, unread, newest
  80 bubbles per chat) beside the session, loads it in `WaDesk::new`, and saves it on a 2 s
  debounce plus on exit. `Command::InitialSync` seeds the sidebar from `session.contacts()` and
  asks for eight history batches; Settings → Chat history re-runs it. Opening Status sends
  `Command::StatusHistory` (`fetch_status_history`), and `Command::Avatar` resolves
  `contact_photo`, downloads it with `ureq` on a thread and caches a circular texture, with
  initials as the fallback.
- **The window repaints every 250 ms.** Session events arrive on other threads through a
  channel; without the timer egui only painted on input, so an idle window could look stuck
  (an empty chat list) until the mouse moved.

## Roadmap (native app)

1. Notifications, tray and autostart
2. AI auto-reply loop (port it out of the removed shell)
3. Voice/video calls on a native audio/video path
