# Parity with the official WhatsApp desktop client

This is the working checklist behind the "match the official client" goal. It is written from the
live client: `web.whatsapp.com` (sampled through the browser MCP: DOM test ids, computed sizes,
colours) and the official desktop screenshots the maintainer sends. **✅** = done and verified,
**🟡** = works but the layout or behaviour still differs, **❌** = missing.

## Shell and navigation

| Official | Ours | State |
| --- | --- | --- |
| Rail: Chats, Calls, Status, Channels, Communities, Meta AI | Chats, Status, Channels, Communities, Settings, own avatar | 🟡 Calls panel missing; Meta AI is replaced by our own AI assistant; Media/You drawer missing |
| Chat list header 64 px, `10px 12px`, `#1D1F1F` | 64 px, 12 px padding, `Palette::elevated` | ✅ |
| Rows 583x72, radius 12 | 72 px rows, 12 px cards inset 6 px | ✅ |
| Custom chat lists: "New list" creates a pill that filters to its chats | `ChatList` in the model (persisted in `chats.json`), `ChatFilter::List(id)`, a **+ New list** pill and one pill per list; the dialog names it and the open chat joins | ✅ (`target/our_lists.png`) |
| Filters All / Unread / Favorites / Communities / Groups + "New list" | the same five in the official order with live counts; Favorites is a real filter (`Model::favourites`, toggled from the chat menu's **Add to / Remove from favourites**, persisted in `chats.json`) | ✅ filter (`target/our_filters_crop.png`); custom lists ❌ |
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
| Reply, react, forward, star, edit, delete | yes | ✅ |
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
| Story viewer with progress bars, reply, react, delete | two/three-segment progress bar at the top, "Name · time" and the close control in the header, the body centred on the dark stage, prev/next by tap zones and arrows; reply/react/delete from the composer row | ✅ chrome (`target/our_status_viewer.png`, `WA_DESK_STATUS_VIEWER=1`) |

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
