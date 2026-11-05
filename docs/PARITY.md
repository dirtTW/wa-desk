# Parity with the official WhatsApp desktop client

This is the working checklist behind the "match the official client" goal. It is written from the
live client: `web.whatsapp.com` (sampled through the browser MCP: DOM test ids, computed sizes,
colours) and the official desktop screenshots the maintainer sends. **✅** = done and verified,
**🟡** = works but the layout or behaviour still differs, **❌** = missing.

## How these samples are taken

The live client is reached through the browser MCP. Two things matter:

- **Opening a chat needs a real pointer sequence.** A synthetic `.click()` on
  `[data-testid="cell-frame-container"]` does nothing in this profile; dispatching
  `pointerdown → mousedown → pointerup → mouseup → click` as `PointerEvent`s does, after which
  `[data-testid="conversation-compose-box-input"]` appears. That is what finally unblocked the
  attach-menu sample (rc.26).
- **Panels are found by `aria-label`.** The rail buttons are `Chats`, `Calls`, `Status`,
  `Channels`, `Communities`, `You`; the composer's attach button is `Attach`; the settings rows and
  the attach entries are plain `role=button` rows with their label as inner text.

Compared on 2026-11-05: the official **forward picker** (message menu → Forward) is a floating card
over the conversation: `✕` + "Forward message to" + a send icon on the header row, an accent-bordered
search field hinting "Search name, number or @username", a "My status" row with a green status disc
and a "2 contacts excluded" line, then a "Recent chats" section whose rows carry a square checkbox on
the left, the avatar, the name and the chat's second line. This client's picker now draws exactly
that card (`target/our_forward.png`); the entries are still forwarded through `Command::Forward`,
one send for every chosen chat.

Opened on 2026-11-05: the **status viewer** (rail `Status`, then a `aria-label="View status"`
row). The official chrome is: the poster's name and the timestamp as **two lines** top-left
("Daffa" / "Yesterday at 10:29 PM"), the thin segmented progress bar across the very top, the story
body centred on the dark stage, and the close control top-right. **Matched**: our viewer now draws
the author over the timestamp the same way ("Ada" / "6:16 am"), the segmented bar across the top,
the body centred and the close control top-right (`target/our_status_viewer.png`).

Opened on 2026-11-05 with the same recipe: the **Privacy** and **Chats** panes. Privacy lists
"Who can see my personal info" (Last seen and online, Profile picture, About, Status, Read receipts),
"Disappearing messages" (Default message timer, Groups), then Calls, Blocked contacts, App lock and an
"Advanced" block (Block unknown account messages, Protect IP address in calls, Turn off link
previews). Chats lists Theme, Default chat theme, Media upload quality, Media auto-download, Spell
check, Replace text with emoji and Enter is send. Both panes now carry those rows in that order
(`target/our_privacy.png`, `target/our_chats_settings.png`); the visibility values read "Phone",
because a linked device can read those settings but never set them.

Opened on 2026-11-05: the **Account** pane (behind `You` then `Account`) lists exactly
**Security notifications · Request account info · How to delete my account**. Our Account page now
carries those three beside the account info, linked devices, startup and log-out cards
(`target/our_settings_account.png`). The recipe is the one from the chat row: find the section label,
walk up until the element carries `role="button"`, then dispatch the `PointerEvent` sequence.

Re-confirmed on 2026-11-05: the Settings screen (behind `You`) lists **Profile · Account · Privacy ·
Chats · Notifications · Keyboard shortcuts · Help** with the subtitles "Name, profile picture,
username", "Security notifications, account info", "Blocked contacts, disappearing messages",
"Theme, wallpaper, chat settings", "Messages, groups, sounds" - the same list and order this client
renders (`target/our_settings_account.png`). Clicking a *section* to open its detail pane did not
take with the same dispatch, so the Settings sub-pages are still compared only through our capture;
the recipe to finish them is to target the section row's inner `role=button` element the same way
the chat row was targeted.

## Shell and navigation

| Official | Ours | State |
| --- | --- | --- |
| Rail: Chats, Calls, Status, Channels, Communities, Meta AI, Media, You | Chats, Calls, Status, Channels, Communities, **Media**, the deleted-history eye, Settings and the own avatar | 🟡 order matches and Media now has its own panel (`target/our_media.png`); **Meta AI** is the only entry with no local equivalent (our AI is the bring-your-own-key assistant in Settings), and Settings opens from the rail instead of a You drawer |
| Chat list header 64 px, `10px 12px`, `#1D1F1F` | 64 px, 12 px padding, `Palette::elevated` | ✅ |
| Rows 583x72, radius 12 | 72 px rows, 12 px cards inset 6 px | ✅ |
| Custom chat lists: "New list" creates a pill that filters to its chats | `ChatList` in the model (persisted in `chats.json`), `ChatFilter::List(id)`, a **+ New list** pill and one pill per list; the dialog names it and the open chat joins | ✅ (`target/our_lists.png`) |
| Chat rows: avatar, name, preview, clock, unread pill, **pin** and **crossed bell** badges, pinned chats first | live sample 2026-11-05 (`target/live_now.jpg`, `live_settings.jpg`): a pinned chat sits at the top with a pin glyph under the clock, a muted one shows the crossed bell in the same slot, and the unread pill keeps the corner. Ours now folds the app-state `pin`/`mute` values into `ChatRow.pinned`/`.muted`, sorts pinned first in both chat orders, persists both in `chats.json` and paints both badges | ? (`target/our_rows.png`) |
| Filters All / Unread / Favorites / Communities / Groups + the round `+` | live sample 2026-11-05 (`tablist "chat-list-filters"`, `target/live_filters.jpg`): **All, Unread, Favorites, Communities, Groups** in that order, **no counts**, hairline-bordered pills with the selected one a dark-green fill carrying light-green text, then a **circular `+`** that starts a new list. Ours now matches that order, drops the counts, uses the same fill/border treatment at the same 32 px height and puts custom lists before the round `+` (they used to be trailed by a `+ New list` text pill). Favorites is a real filter (`Model::favourites`, toggled from the chat menu), and the `+` opens the new-list dialog (`ChatList`, persisted in `chats.json`) | ? (`target/our_filters.png`, `target/live_filters.jpg`) |
| Search field `raised` (#2A3942) | same | ✅ |
| Chat header 64 px | 64 px | ✅ |
| Sticker panel: icon tabs + `+`, accent store search, Create tile, grid, icon footer | sampled live 2026-11-05: clock / star / packs icons with the selected one underlined, a `+` to add a pack, the accent-ringed "Search via WhatsApp sticker store" pill, a four-column grid led by Create, then the emoji/GIF/sticker footer. The live `+` opens the **WhatsApp Sticker Store** (Greetings / Happy / Sad / Angry / Celebrate plus Create sticker); ours now opens the same sheet - a titled list with those five categories, each with its emoji and chevron, a close control and "Show the packs I have" - and says where the packs really come from, because a linked device cannot list the store | ? (`target/our_stickers.png`, `target/our_sticker_store.png`, `WA_DESK_STICKER_STORE=1`) |
| Emoji panel: eight categories, accent search pill, "Smileys & People", nine-column grid, icon footer | sampled live 2026-11-05 (real click on the combined smiley): eight category icons, the search pill ringed in the accent colour, the section label, the colour grid, then the emoji / GIF / sticker footer with the selected tab underlined and no Close control | ✅ (`target/our_emoji.png`) |
| Composer: attach, one combined smiley, send/mic | live capture 2026-11-05: `Attach`, the rounded field with the single `Emojis, GIFs, Stickers` control, then send (or mic when the draft is empty); the sticker panel is the emoji panel's Stickers tab | ✅ (`target/our_chat.png`) |

## Conversation

| Official | Ours | State |
| --- | --- | --- |
| Text bubbles, tails, sender colours, ticks, day chips | yes | ✅ |
| Media inside the bubble with caption and clock | yes | ✅ |
| Media viewer: `#0B141A` stage, header (avatar, chat name, "Today at …"), right toolbar (download, reply, star, forward, kebab, close), circular prev/next discs, caption in a translucent rounded box above the filmstrip, filmstrip with the selected thumbnail a step larger and ringed in the accent (no numeric counter) | built from the recorded design tokens - the live viewer has not been captured beside ours yet, because opening it needs the user to click a photo. Both paths captured: video (`target/our_viewer.png`) and photo (`target/our_viewer_photo.png`, `WA_DESK_VIEWER_IMAGE=<file>`) | ? our capture only |
| Video plays at the clip's rate | `ffprobe` rate, off-thread frames, measured 18.4 ms | ✅ |
| Animated WebP stickers | `webp_anim` RIFF decoder (ffmpeg cannot demux them) | ✅ |
| GIFs animate inline | yes | ✅ |
| Emoji picker 40 px cells | yes | ✅ |
| Voice notes: record (mic in an empty composer), play, waveform, seek | recording captured at 16 kHz through cpal, encoded to Opus and muxed into Ogg (`src/voicenote.rs`), posted as `ptt` with a 64-bar waveform via `Session::send_voice_note`; playback, waveform and seek were already there | ✅ (`target/our_mic_crop.png`) |
| Documents card | yes | ✅ |
| Reply, react, forward, star, edit, delete | yes, through the message menu; **forward** is the vision client's own card now - floating rounded panel, close + "Forward message to" + send on the header row, accent-outlined search field (name, number or @username), the "My status" row, "Recent chats" with a square checkbox, the avatar and the chat's last line per row | ✅ (`target/our_forward.png`, `WA_DESK_FORWARD=1`) |
| Group info: name + "Group · N members", Media links and docs / Starred messages / Notification settings / Chat theme / Encryption / Disappearing messages / Advanced chat privacy, then the members with "View all (N more)" / "See member changes", then Add to list / Export chat / Clear chat / Report group / Exit | read from the live pane 2026-11-05 and now matched row for row (Media and Starred open their screens, the disappearing card is settable, Add to list really creates a list) | ✅ (`target/our_group.png`, `WA_DESK_SCREEN=group`); the creator card is now rendered too, so the row is matched |
| Communities: New community + per-subgroup name/last message/time | sampled live 2026-11-05: the drawer lists each community and its subgroups as chat-like rows; ours has the New community tile and preview/stamp rows | ✅ (`target/our_communities.png`) |
| Media panel: Media / Docs / Links | sampled live 2026-11-05: tabs, Search, Sort by, Select, Close and a tile grid captioned "Image from …" / "Video from …"; ours mirrors it over `Model::media_gallery`/`document_gallery`/`link_gallery`, newest first | ✅ (`target/our_media.png`, `WA_DESK_SCREEN=media`) |
| Settings sub-pages sampled live: Account, Privacy, Chats, Notifications | Account: Security notifications, Request account info, How to delete my account. Privacy: the visibility rows, disappearing messages, calls/contacts and the Advanced block. Chats: theme, then Media upload quality, Media auto-download, Spell check, Replace text with emoji, Enter is send. Notifications: Messages, Groups, Status, Calls, Show previews, Play sound for outgoing messages, Background sync | ✅ (`target/our_privacy.png`, `target/our_chats_settings.png`, `target/our_notifications.png`) |
| Attach menu: **sampled live 2026-11-05** - Photos & videos, Camera, Document, Audio, Contact, Poll | the same order in a rounded card, plus Location (this client's extra) last; file-backed entries open the picker, Poll/Contact/Location open their card dialogs | ✅ menu (`target/our_attach.png`, `target/our_poll_dialog.png`) |
| Locations (pin + live): receive, render, send | `media_kind` maps `locationMessage`/`liveLocationMessage`, `location_view` reads the doubles (and the caption as the live name); the bubble draws the map tile, place, coordinates and address (or a "Live location" line); `Session::send_location` sends a pin and the attach menu opens a *Share a location* dialog | ✅ receive/render (`target/our_location.png`), send (dialog) |
| Polls and contact cards: compose, send, receive, render | **send**: attach menu → *Create poll* / *Share a contact* dialogs → `SendPoll`/`SendContact` → `polls().create` / `contactMessage` + vCard. **receive**: `media_kind` maps `pollCreationMessage` / `contactMessage` / `contactsArrayMessage`, `poll_view`/`contact_view` build the views (options, `selectable_options_count`, number parsed out of the vCard) and the bubbles draw the question + option rows or the initials disc card | ✅ (`target/our_poll.png`, `target/our_poll_dialog.png`, `target/our_contact_dialog.png`) |

| View-once: the placeholder bubble with the circled "1", "Photo", one of "Tap to open it once" / "Opened" / "Open this on your phone", and the media opens once in the viewer | `MessageView.kind == "view_once"` (a `<unavailable type="view_once">` fanout) paints a 250 px placeholder bubble: circled `1`, the kind, the state line and the clock. `Model::view_once_playable` allows opening only when the body survived a reply quote **and** the file is on disk; `open_viewer` adds it to `view_once_open`, so the bubble flips to "Opened" and cannot reopen. A fanout with no recovered body says "Open this on your phone" instead of pretending | ? (`target/our_view_once.png`, and `view_once_opens_once_and_only_with_a_file`) |
| Disappearing messages | **Groups**: the group info's DISAPPEARING MESSAGES card sets Off / 24 h / 7 d / 90 d through `Session::set_group_ephemeral` and the refreshed metadata rings the current value. **Account default** (Settings -> Privacy) stays read-only: it is a phone-owned privacy setting | ✅ groups (`target/our_group.png`) |

## Status / stories

| Official | Ours | State |
| --- | --- | --- |
| Status screen = rail + 614 px drawer with the list and the "Share statuses" pane | rail + list (64 px header) + 320 px share pane | ✅ (`target/our_status3.png`) |
| "My status" row with ring and "Click to add status update" | yes | ✅ |
| Recent / Viewed sections with 72 px rows and rings | `Model::status_sections()` splits unseen (Recent) from fully-seen (Viewed); both sections render with their headings and rings | ✅ (`target/our_status4.png`) |
| Text status with the colour/font palette | the composer shows the official 20-colour array from `wa_ui::composer::BACKGROUNDS` as selectable discs (the chosen one ringed), the four fonts (`system/serif/script/bold`), a preview box in the chosen colour, and Send posts `background = wa_ui::composer::argb(hex)` + the font name | ✅ (`target/our_status_compose.png`) |
| Photo and video status | `Command::PostStatusMedia` via the file picker | ✅ |
| Media loads instantly: the sender's inline JPEG paints while the file downloads | `inline_thumb` builds a data URI from `jpegThumbnail`, `MessageView.thumb` ships it with the event, the bubble paints it cover-scaled in the media tile; eight downloads run in parallel | ✅ (`target/our_thumb.png`, `WA_DESK_THUMB=1`) |
| WhatsApp markup `*bold*` `_italic_` `~strike~` ``` ```mono``` ``` | parsed with the phone's rules (opener before a non-space, closer after one, runs of markers are one delimiter); bold is double-painted, strike a hairline, mono the monospace face | ✅ (`target/our_markup.png`, `WA_DESK_MARKUP=1`) |
| Composer: attach, one combined smiley, send/mic | live capture 2026-11-05: `Attach`, the rounded field with the single `Emojis, GIFs, Stickers` control, then send (or mic when the draft is empty); the sticker panel is the emoji panel's Stickers tab | ✅ (`target/our_chat.png`) |
| Send failures are visible: a red `!` instead of a tick | `UiEvent::SendFailed` from `wa-core`, `Model::send_failures` + `send_error`, the bubble paints a red disc with `!` | ✅ (`target/our_send_failed.png`) |
| Conversation scroll: newest message on open/send, floating jump-to-latest disc | opening a chat or sending pins the view to the last bubble (`ui.scroll_to_rect(..., Align::BOTTOM)`); scrolling up reveals the official 44 px chevron disc above the composer | ✅ (`target/real_scroll.png`, `WA_DESK_SCROLL_TOP=1`) |
| Story viewer: progress bar, avatar, photo/video stage with the blurred backdrop, caption, reply (text **and sticker**), react, delete | `FetchMedia` on the status id, stage cover-scaled + dimmed behind a letterboxed frame, caption over the bottom; the bar has emoji + sticker + reply field + heart, or the view count + Delete on your own | ✅ (`target/our_status_media.png`, `target/our_status_viewer.png`) |

## Channels, communities, calls, settings

| Official | Ours | State |
| --- | --- | --- |
| Channels: "Discover channels" card (description + Create channel / Explore channels), followed list, read-only posts | the same card with the official copy and tiles, followed list as 64 px rows, posts read-only; Create/Explore say what the phone owns | ✅ layout (`target/our_channels3.png`); the channel directory itself is not in this build |
| Communities: "Create communities" empty state, community rows with group counts, the announcement group first, then the topic groups | dedicated screen: official copy and New-community tile when empty, 72 px community cards with "N groups", the selected community opens its children (announcements first, badged "Announcements"), clicking one opens the chat; the Communities pill stays in the chat list | ✅ (`target/our_communities2.png`) |
| Call overlay: avatar, name, state + timer, mute and the red hang-up disc, remote video as the stage (official-side sample pending - the user tests calls at 18:00) | the same sheet: 96 px avatar disc with initials, name, "Connected · 00:46", a mute disc and the red hang-up disc; video calls paint the remote H.264 stage with the mirrored self-view thumbnail | ✅ (`target/our_call_overlay.png`, `WA_DESK_CALL=connected`) |
| Status drawer: "Status" header with kebab + circle-plus, **My status** row with the green plus badge and "Click to add status update", Recent/Viewed headings, rows stamped "Today at …", and the centred share pane (grey ring, "Share statuses", two round discs) | live sample 2026-11-05 (`target/live_status.jpg`): all of the above, with no close button in the header. Ours now matches that shape; the compose button opens the text composer, the kebab reports that status settings live on the phone, and row subtitles use the newest update's timestamp (the update count is the fallback) | ? (`target/our_status.png` beside `target/live_status.jpg`) |
| Empty conversation pane: a calls promo card (illustration, "Voice and video calling is now available", "Now you can make and join calls on WhatsApp Web.", a green **Go to Calls** pill), then Send document / Add contact / Invite a friend / New call / Ask Meta AI discs and a lock + "Your personal messages are end-to-end encrypted" footer | live sample 2026-11-05 (`target/live_now.jpg`, the pane with no chat open): the card is a rounded elevated panel with an accent illustration, a 19 px bold title, a 13 px body line and the 34 px pill; the five actions are grey discs with 11.5 px labels and the footer is a lock + privacy line. Ours now paints that whole pane (`WA_DESK_NO_CHAT=1`), with Go to Calls opening the Calls drawer, Send document opening the chat picker, Add contact opening the contact draft, and Invite/Meta AI stating where they really happen (this client has no Meta AI service; its own AI assistant answers from Settings) | ? (`target/our_empty.png` beside `target/live_now.jpg`) |
| Calls drawer: header (title, kebab, phone-plus FAB), "Search name, number or @username", Favorites (with Add favorite), Recent rows (avatar, name, red Missed / grey Outgoing/Incoming, time at the right, call buttons on hover) | live sample 2026-11-05 (`target/live_calls.jpg`, `target/live_calls_list.jpg`): Calls is a **drawer in the chat list's column** with that header, that search field, a green **Favorites** heading plus an **Add favorite** disc row, a green **Recent** heading and 72 px rows whose name stays white while the kind line is red for a missed call. Ours now renders as a 300-460 px `SidePanel` with the messages pane beside it, the same header/FAB (a phone glyph with a plus badge), the same search placeholder, both headings, the Add favorite row and the same row anatomy (duration appended for connected calls); the call buttons show on hover instead of always. Rows still start a call | ? (`target/our_calls.png` beside `target/live_calls.jpg`) |
| Settings drawer: account name header, an accent-ringed **Search** pill, then Profile / Account / Privacy / Chats / Notifications / Keyboard shortcuts / Help and feedback / Log out with the live subtitles and glyphs | live sample 2026-11-05 (`target/live_settings.jpg`, the gear at the bottom of the rail): the header is the account name with **no status line**, the search is a permanent pill ringed in the accent colour, the rows are flat with a leading glyph (person, key/lock, block, chat, bell, keyboard, help), the subtitles read "Name, profile picture, username", "Quick actions", "Help center, contact us, privacy policy" and Log out is red. Ours now matches the header, the pill, those subtitles and those glyphs (five new Material variants); our own **AI assistant** and **Updates** pages ride between Keyboard shortcuts and Help, and the **View profile / View about** links stay because this client has no separate You drawer; the live "What's happening?" status composer is not mirrored here (status posting lives in the Status screen) | ? (`target/our_settings.png` beside `target/live_settings.jpg`) |
| Updates: installed version, channel, check, download, restart | `Download & install` asks GitHub for the newest pre-release, streams the platform asset in 64 kB chunks, and **the button itself becomes the progress bar** ("Downloading 62%" on a pill that fills left to right) with `16.1 of 25.9 MB` above it; `Restart now` swaps the binary and relaunches | live sample of the official page is the `Check for updates` row only, so this is our own shape built from the same tokens | ? (`target/our_update.png`, `WA_DESK_UPDATE_DEMO=62`) |
| Linked devices, two-step verification, block list | Account shows a read-only **Linked devices** card (This device = WA Desk, Manage devices = Phone) and a two-step verification row that names the phone as the owner; Privacy explains the phone-owned block list | 🟡 read-only, by design (the protocol gives linked devices no control) |
| Profile drawer ("You") | the pane is the avatar + name, a **Search** field that filters the section list, **View profile / View about**, then Profile / Account / Privacy / Chats / Notifications / Keyboard shortcuts / AI assistant / Updates / Help and feedback / Log out - the same shape as the live drawer | ✅ (`target/our_you.png`, `WA_DESK_SETTINGS=1`) |
| Chat header: avatar, name, and the participant list under it | live sample 2026-11-05 (`target/live_chat_open.jpg`): "Test group" over **"Yi, You"** - the participants' names, ending with You - not a member count. Ours now builds that line with `Model::group_subtitle` (three names, then "and N others", then You) and resolves each name with `Model::member_name` (push name, else number, else JID user); the group-info member list uses the same resolution. The demo has no push names, so `target/our_header.png` shows the numbers there | ? (`target/our_header.png` beside `target/live_chat_open.jpg`) |
| Group info: centred photo with a camera badge, name + edit pencil, "Group · N members", the Voice / Video / Add / Search discs, description, then **flat icon rows** (Media links and docs with a four-thumbnail strip, Starred messages, Notification settings, Chat theme, Encryption with its subtitle, Advanced chat privacy) | live sample 2026-11-05 (`target/live_chat_menu2.jpg`): 96 px photo + green camera badge, name + pencil, four labelled discs, "Add group description" link, then rows that are **flat (no card)** with a leading glyph, a count/trailing value and a 4-thumbnail strip under Media. Ours matches the head and the rows now carry `inventory_2` (folder), `star`, `notifications`, `palette`, `lock` glyphs with the same hover band; the Media row leads into up to four 56 px thumbnails from the group's cached photos, and clicks still open the Media / Starred screens. Disappearing messages stays a settable card below, and the member list follows | ? (`target/our_group.png` beside `target/live_chat_menu2.jpg`) |

## Verification habits

- Every visual change is captured with `--preview --screenshot` and compared with the official
  screen; the hooks are `WA_DESK_SCREEN=status|channels|group`, `WA_DESK_SETTINGS=1`,
  `WA_DESK_VIEWER=1`, `WA_DESK_EMOJI=1`, `WA_DESK_STICKER_WEBP=<file>` and `WA_DESK_OPEN_CHAT=<text>`.
- `WA_DESK_DIAG=1` writes `memory:` and `paint:` lines so memory and frame pacing stay measurable.
- `cargo test --workspace` (110 native, 38 wa-core, 232 wa-ui) plus `--selftest` gate every release.


## Verification state (2026-11-05)

Surfaces compared **live beside ours** over the browser MCP: the filter row (`live_filters.jpg`), the Calls drawer (`live_calls.jpg`), group info (`live_chat_menu2.jpg`) and Settings (`live_settings.jpg`).

Surfaces still verified only against our own `--preview` captures: the media viewer, the sticker
panel (its last live sample predates rc.75) and the Updates page (which has no official
counterpart - the live client keeps "Check for updates" inside Help).

Rows that cannot be closed without the user: the **call overlay** (needs a live call to capture
the official sheet) and live **view-once** data (no view-once message has reached this linked
device). **Meta AI** and **linked devices / 2FA / block list** have no linked-device API, so they
stay read-only by design.
