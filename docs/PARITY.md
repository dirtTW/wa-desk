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
| Rail: Chats, Calls, Status, Channels, Communities, Meta AI, Media, You | Chats, Calls, Status, Channels, Communities, the deleted-history eye, Settings and the own avatar | 🟡 order and badges match for the shared entries; **Meta AI** and **Media** have no local equivalent (our AI is the bring-your-own-key assistant in Settings), and Settings opens from the rail instead of a You drawer (`target/real_scroll.png`) |
| Chat list header 64 px, `10px 12px`, `#1D1F1F` | 64 px, 12 px padding, `Palette::elevated` | ✅ |
| Rows 583x72, radius 12 | 72 px rows, 12 px cards inset 6 px | ✅ |
| Custom chat lists: "New list" creates a pill that filters to its chats | `ChatList` in the model (persisted in `chats.json`), `ChatFilter::List(id)`, a **+ New list** pill and one pill per list; the dialog names it and the open chat joins | ✅ (`target/our_lists.png`) |
| Filters All / Unread / Favorites / Communities / Groups + "New list" | the same five in the official order with live counts; Favorites is a real filter (`Model::favourites`, toggled from the chat menu's **Add to / Remove from favourites**, persisted in `chats.json`) and **New list** creates custom lists (`ChatList`, `ChatFilter::List`) | ✅ filter + custom lists (`target/our_filters_crop.png`, `target/our_lists.png`) |
| Search field `raised` (#2A3942) | same | ✅ |
| Chat header 64 px | 64 px | ✅ |
| Composer 62 px bar, 38 px field, icons inside | same | ✅ |

## Conversation

| Official | Ours | State |
| --- | --- | --- |
| Text bubbles, tails, sender colours, ticks, day chips | yes | ✅ |
| Media inside the bubble with caption and clock | yes | ✅ |
| Media viewer: header, toolbar, arrows, transport overlay, filmstrip | rebuilt 1:1 | ✅ |
| Video plays at the clip's rate | `ffprobe` rate, off-thread frames, measured 18.4 ms | ✅ |
| Animated WebP stickers | `webp_anim` RIFF decoder (ffmpeg cannot demux them) | ✅ |
| GIFs animate inline | yes | ✅ |
| Emoji picker 40 px cells | yes | ✅ |
| Voice notes: record (mic in an empty composer), play, waveform, seek | recording captured at 16 kHz through cpal, encoded to Opus and muxed into Ogg (`src/voicenote.rs`), posted as `ptt` with a 64-bar waveform via `Session::send_voice_note`; playback, waveform and seek were already there | ✅ (`target/our_mic_crop.png`) |
| Documents card | yes | ✅ |
| Reply, react, forward, star, edit, delete | yes, through the message menu; **forward** is the vision client's own card now - floating rounded panel, close + "Forward message to" + send on the header row, accent-outlined search field (name, number or @username), the "My status" row, "Recent chats" with a square checkbox, the avatar and the chat's last line per row | ✅ (`target/our_forward.png`, `WA_DESK_FORWARD=1`) |
| Settings sub-pages sampled live: Account, Privacy, Chats, Notifications | Account: Security notifications, Request account info, How to delete my account. Privacy: the visibility rows, disappearing messages, calls/contacts and the Advanced block. Chats: theme, then Media upload quality, Media auto-download, Spell check, Replace text with emoji, Enter is send. Notifications: Messages, Groups, Status, Calls, Show previews, Play sound for outgoing messages, Background sync | ✅ (`target/our_privacy.png`, `target/our_chats_settings.png`, `target/our_notifications.png`) |
| Attach menu: **sampled live 2026-11-05** - Photos & videos, Camera, Document, Audio, Contact, Poll | the same order in a rounded card, plus Location (this client's extra) last; file-backed entries open the picker, Poll/Contact/Location open their card dialogs | ✅ menu (`target/our_attach.png`, `target/our_poll_dialog.png`) |
| Locations (pin + live): receive, render, send | `media_kind` maps `locationMessage`/`liveLocationMessage`, `location_view` reads the doubles (and the caption as the live name); the bubble draws the map tile, place, coordinates and address (or a "Live location" line); `Session::send_location` sends a pin and the attach menu opens a *Share a location* dialog | ✅ receive/render (`target/our_location.png`), send (dialog) |
| Polls and contact cards: compose, send, receive, render | **send**: attach menu → *Create poll* / *Share a contact* dialogs → `SendPoll`/`SendContact` → `polls().create` / `contactMessage` + vCard. **receive**: `media_kind` maps `pollCreationMessage` / `contactMessage` / `contactsArrayMessage`, `poll_view`/`contact_view` build the views (options, `selectable_options_count`, number parsed out of the vCard) and the bubbles draw the question + option rows or the initials disc card | ✅ (`target/our_poll.png`, `target/our_poll_dialog.png`, `target/our_contact_dialog.png`) |

| View-once | partially (rail eye reveals recovered ones) | 🟡 |
| Disappearing messages | indicator shown | 🟡 settings toggle not wired |

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
| Composer: Enter sends, and the bubble appears at once | `enter_sends` samples the key before the single-line field consumes it; `WA_DESK_ENTER_TEST=1` drives a real `Event::Key` through egui for the capture | ✅ (`target/our_enter_send.png`) |
| Send failures are visible: a red `!` instead of a tick | `UiEvent::SendFailed` from `wa-core`, `Model::send_failures` + `send_error`, the bubble paints a red disc with `!` | ✅ (`target/our_send_failed.png`) |
| Conversation scroll: newest message on open/send, floating jump-to-latest disc | opening a chat or sending pins the view to the last bubble (`ui.scroll_to_rect(..., Align::BOTTOM)`); scrolling up reveals the official 44 px chevron disc above the composer | ✅ (`target/real_scroll.png`, `WA_DESK_SCROLL_TOP=1`) |
| Story viewer with progress bars, reply, react, delete | **header sampled live 2026-11-05**: name on one line with the timestamp under it, segmented progress bar across the top, body centred, close top-right; reply/react/delete from the composer row, prev/next by tap zones and arrows | ✅ chrome (`target/our_status_viewer.png`, `WA_DESK_STATUS_VIEWER=1`) |

## Channels, communities, calls, settings

| Official | Ours | State |
| --- | --- | --- |
| Channels: "Discover channels" card (description + Create channel / Explore channels), followed list, read-only posts | the same card with the official copy and tiles, followed list as 64 px rows, posts read-only; Create/Explore say what the phone owns | ✅ layout (`target/our_channels3.png`); the channel directory itself is not in this build |
| Communities: "Create communities" empty state, community rows with group counts, the announcement group first, then the topic groups | dedicated screen: official copy and New-community tile when empty, 72 px community cards with "N groups", the selected community opens its children (announcements first, badged "Announcements"), clicking one opens the chat; the Communities pill stays in the chat list | ✅ (`target/our_communities2.png`) |
| Call overlay: avatar, name, state + timer, mute and the red hang-up disc, remote video as the stage | the same sheet: 96 px avatar disc with initials, name, "Connected · 00:46", a mute disc and the red hang-up disc; video calls paint the remote H.264 stage with the mirrored self-view thumbnail | ✅ (`target/our_call_overlay.png`, `WA_DESK_CALL=connected`) |
| Calls: history panel, start/accept/end, mute, self view | rail **Calls** screen with a 64 px header and 72 px rows (direction glyph, date, kind, duration, red for missed, call button that dials that contact); `Model::call_log` records every finished call (outgoing/missed/duration) | ✅ (`target/our_calls.png`) |
| Settings: Profile / Account / Privacy / Chats / Notifications / Keyboard shortcuts / Help | the same list in the official order with its subtitles, plus AI and Updates; Account holds the phone number, session, security rows, autostart and Log out, Privacy the disappearing-message and block-list notes, Notifications the toast switch and a test, Shortcuts the key table, Help the version and storage path | ✅ (`target/our_settings_account.png`) |
| Linked devices, two-step verification, block list | Account shows a read-only **Linked devices** card (This device = WA Desk, Manage devices = Phone) and a two-step verification row that names the phone as the owner; Privacy explains the phone-owned block list | 🟡 read-only, by design (the protocol gives linked devices no control) |
| Profile drawer ("You") | settings sections + own avatar | 🟡 |
| Group info: avatar, subject, member count, description, actions, admin badges, exit | rebuilt in the official shape | ✅ (`target/our_group2.png`) |

## Verification habits

- Every visual change is captured with `--preview --screenshot` and compared with the official
  screen; the hooks are `WA_DESK_SCREEN=status|channels|group`, `WA_DESK_SETTINGS=1`,
  `WA_DESK_VIEWER=1`, `WA_DESK_EMOJI=1`, `WA_DESK_STICKER_WEBP=<file>` and `WA_DESK_OPEN_CHAT=<text>`.
- `WA_DESK_DIAG=1` writes `memory:` and `paint:` lines so memory and frame pacing stay measurable.
- `cargo test --workspace` (110 native, 38 wa-core, 232 wa-ui) plus `--selftest` gate every release.
