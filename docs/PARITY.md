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
| Filters All / Unread / Favorites / Communities / Groups + "New list" | All / Groups N / Communities N / Unread N | 🟡 no Favorites, no custom lists; we show counts instead |
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
| Channels: followed list + Discover | followed channels panel, read-only posts | 🟡 no Discover/explore list |
| Communities: dedicated screen with announcement groups | Communities is only a chat filter | ❌ |
| Calls: history panel, start/accept/end, mute, self view | rail **Calls** screen with a 64 px header and 72 px rows (direction glyph, date, kind, duration, red for missed, call button that dials that contact); `Model::call_log` records every finished call (outgoing/missed/duration) | ✅ (`target/our_calls.png`) |
| Settings: Account / Privacy / Chats / Notifications / Storage / Shortcuts / Help | General / Chats / AI / Updates / Help | 🟡 account/privacy/storage pages missing |
| Linked devices, two-step verification, block list | — | ❌ |
| Profile drawer ("You") | settings sections + own avatar | 🟡 |

## Verification habits

- Every visual change is captured with `--preview --screenshot` and compared with the official
  screen; the hooks are `WA_DESK_SCREEN=status|channels|group`, `WA_DESK_SETTINGS=1`,
  `WA_DESK_VIEWER=1`, `WA_DESK_EMOJI=1`, `WA_DESK_STICKER_WEBP=<file>` and `WA_DESK_OPEN_CHAT=<text>`.
- `WA_DESK_DIAG=1` writes `memory:` and `paint:` lines so memory and frame pacing stay measurable.
- `cargo test --workspace` (110 native, 38 wa-core, 232 wa-ui) plus `--selftest` gate every release.
