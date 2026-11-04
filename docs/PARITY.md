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
| Voice notes: play, waveform, seek | play/seek/waveform | 🟡 recording a voice note is not wired |
| Documents card | yes | ✅ |
| Reply, react, forward, star, edit, delete | yes | ✅ |
| Polls, location, live location, contact card | — | ❌ |
| View-once | partially (rail eye reveals recovered ones) | 🟡 |
| Disappearing messages | indicator shown | 🟡 settings toggle not wired |

## Status / stories

| Official | Ours | State |
| --- | --- | --- |
| Status screen = rail + 614 px drawer with the list and the "Share statuses" pane | rail + list (64 px header) + 320 px share pane | ✅ (`target/our_status3.png`) |
| "My status" row with ring and "Click to add status update" | yes | ✅ |
| Recent / Viewed sections with 72 px rows and rings | rows + rings; a single list, no section split | 🟡 |
| Text status with the colour/font palette | `Command::PostStatusText` (green, system font) | 🟡 palette UI not exposed |
| Photo and video status | `Command::PostStatusMedia` via the file picker | ✅ |
| Story viewer with progress bars, reply, react, delete | yes | ✅ |

## Channels, communities, calls, settings

| Official | Ours | State |
| --- | --- | --- |
| Channels: "Discover channels" card (description + Create channel / Explore channels), followed list, read-only posts | the same card with the official copy and tiles, followed list as 64 px rows, posts read-only; Create/Explore say what the phone owns | ✅ layout (`target/our_channels3.png`); the channel directory itself is not in this build |
| Communities: dedicated screen with announcement groups | Communities is only a chat filter | ❌ |
| Calls: history panel, start/accept/end, mute, self view | rail **Calls** screen with a 64 px header and 72 px rows (direction glyph, date, kind, duration, red for missed, call button that dials that contact); `Model::call_log` records every finished call (outgoing/missed/duration) | ✅ (`target/our_calls.png`) |
| Settings: Profile / Account / Privacy / Chats / Notifications / Keyboard shortcuts / Help | the same list in the official order with its subtitles, plus AI and Updates; Account holds the phone number, session, security rows, autostart and Log out, Privacy the disappearing-message and block-list notes, Notifications the toast switch and a test, Shortcuts the key table, Help the version and storage path | ✅ (`target/our_settings_account.png`) |
| Linked devices, two-step verification, block list | — | ❌ |
| Profile drawer ("You") | settings sections + own avatar | 🟡 |
| Group info: avatar, subject, member count, description, actions, admin badges, exit | rebuilt in the official shape | ✅ (`target/our_group2.png`) |

## Verification habits

- Every visual change is captured with `--preview --screenshot` and compared with the official
  screen; the hooks are `WA_DESK_SCREEN=status|channels|group`, `WA_DESK_SETTINGS=1`,
  `WA_DESK_VIEWER=1`, `WA_DESK_EMOJI=1`, `WA_DESK_STICKER_WEBP=<file>` and `WA_DESK_OPEN_CHAT=<text>`.
- `WA_DESK_DIAG=1` writes `memory:` and `paint:` lines so memory and frame pacing stay measurable.
- `cargo test --workspace` (110 native, 38 wa-core, 232 wa-ui) plus `--selftest` gate every release.
