/* Replace every inline SVG icon in the WA Desk UI with the official
   Material Symbols (outlined, 24px) artwork downloaded from
   fonts.google.com/icons (fonts.gstatic.com).

   - Keys are distinctive prefixes of the OLD first-path `d`; longest match wins.
   - Only the paths inside each <svg> are swapped; classes/ids/aria stay, so
     all existing CSS selectors and JS hooks keep working.
   - fill="none"/stroke attributes are dropped from replaced svgs (Material
     artwork is fill-based).
   - Unresolved svgs are reported and left untouched.
   Usage: node scripts/apply-material-icons.mjs
*/
import fs from "node:fs";
import path from "node:path";

const paths = JSON.parse(
  fs.readFileSync("scripts/material-paths.json", "utf8"),
);

// old first-path d prefix → material name
const MAP = {
  // chat list / chat menu
  "M12 22a2.5 2.5 0 0 0 2.45-2h-4.9A2.5 2.5 0 0 0 12 22zm7.53-2.06L4.06 4.47": "notifications_off",
  "M16 3a1 1 0 0 1 .92 1.39l-1.13 2.83 3.05 3.05a1 1 0 0 1-.72 1.7l-3.47-.01": "push_pin",
  "M12 22a2.5 2.5 0 0 0 2.45-2h-4.9A2.5 2.5 0 0 0 12 22zm7-9h-2V10.5c0-3.3": "notifications",
  "m19.8 18.4-1.8-1.8V6c0-1.1-.9-2-2-2H6.4L4.6 2.2l-1.4 1.4-.1.1 16.7 16.7": "mark_chat_unread",
  "M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z": "mark_chat_read",
  "M3 3h18a1 1 0 0 1 1 1v4a1 1 0 0 1-.6.9L20 10v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-9": "inventory_2",
  "M20 6H10v2h10V6zm-8 6h10v2H12v-2zM10 18h10v2H10v-2zM4 6h2v2H4V6zm0 6h2v2H4v-2zm0 6h2v2H4v-2z": "format_list_bulleted",
  "M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2zm0 2a7.9 7.9 0 0 1 4.9 1.7L6.7 16.9": "block",
  "M22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6z": "mail",
  "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM8 9h8v10H8V9zm7.5-5-1-1h-5l-1 1H5v2h14V4h-3.5z": "delete",
  "M6 19a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z": "delete",

  // chat rendering
  "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 15-4-4": "schedule",
  "M14 9V5l7 7-7 7v-4c-4.5 0-7.6 1.4-10 4.5.9-4.6 3.6-8.5 10-10.5z": "shortcut",
  "M13 6l6 6-6 6v-3.2c-4 0-6.8 1-8.7 3.2C5.6 12.4 9.2 9.3 13 8.7V6z": "shortcut",
  "M10 9V5l-7 7 7 7v-4.1c5 0 8.5 1.6 11 5.1-1-5-4-10-11-11z": "reply",
  "M10 8.26V7a1 1 0 0 0-1.7-.7l-4 4a1 1 0 0 0 0 1.4l4 4A1 1 0 0 0 10 15.2v-2.1c3.7 0 6.2 1 8 3.6.5.7 1.8.3 1.5-.6A8.4 8.4 0 0 0 10 8.3z": "reply",
  "M10 8.26V7a1 1 0 0 1 1.7-.7l4 4a1 1 0 0 1 0 1.4l-4 4A1 1 0 0 1 10 15.2v-2.1c-3.7 0-6.2 1-8 3.6-.5.7-1.8.3-1.5-.6A8.4 8.4 0 0 1 10 8.3z": "shortcut",

  // core.js avatars + media kinds
  "M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3Zm-8 0c1.66 0": "group",
  "M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5C15 14.": "group",
  "M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm8 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5C17.67": "person",
  "M21 19V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2zM8.5 11.5l2.5 3": "image",
  "M17 10.5V7a1 1 0 0 0-1-1H4a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-3.5l4 4v-11l-4 4z": "videocam",
  "M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.92V21h2v-3.08A7 7 0 0 0 19 11h-2z": "mic",
  "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8": "description",
  "M20 12a8 8 0 1 0-8 8h5l3-3v-5zm-9 3.5a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3zm5 0a1.5 1.5 0 1 1 0-3": "sticker",
  "M12 1a9 9 0 0 0-9 9v7c0 1.66 1.34 3 3 3h3v-8H5v-2c0-3.87 3.13-7 7-7s7 3.13 7 7v2h-4v8h3c1.66 0 3-1.34 3-3v-7a9 9 0 0 0-9-9z": "headphones",

  // rail + sidebar chrome
  "M20 2H4a2 2 0 0 0-2 2v18l4-4h14a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2z": "chat_bubble",
  "M20 2H4a2 2 0 0 0-2 2v18l4-4h14a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2Zm-4 10H8v-2h8v2Zm0-3H8V7h8v2Z": "forum",
  "M6.62 10.79a15.05 15.05 0 0 0 6.59 6.59l2.2-2.2a1 1 0 0 1 1.02-.24c1.12.37 2.33.57 3.57.57": "call",
  "M12 2a10 10 0 1 0 10 10h-2a8 8 0 1 1-8-8V2z": "donut_large",
  "M4 6h16v2H4V6zm0 5h16v2H4v-2zm0 5h10v2H4v-2zM18.5 14.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z": "campaign",
  "M12 4.5C7 4.5 2.7 7.6 1 12c1.7 4.4 6 7.5 11 7.5s9.3-3.1 11-7.5c-1.7-4.4-6-7.5-11-7.5zm0 12.5a5 5 0 1 1 0-10": "visibility",
  "M12 5C7 5 2.7 9.2 1.5 12 2.7 14.8 7 19 12 19s9.3-4.2 10.5-7C21.3 9.2 17 5 12 5Zm0 12a5 5 0 1 1 0-10": "visibility",
  "M17 7l-1.41 1.41L18.17 11H8v2h10.17l-2.58 2.58L17 17l5-5-5-5zM4 5h8V3H4c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h8v-2H4V5z": "logout",
  "M19.14 12.94a7.07 7.07 0 0 0 0-1.88l2.03-1.58": "settings",
  "M12.51 14.4c-.66-.66-1.4-1.24-2.21-1.71a10.14 10.14 0 0 0-4.9-1.55": "edit_square",
  "M12 8a2 2 0 1 0 0-4 2 2 0 0 0 0 4zm0 2a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm0 6a2 2 0 1 0 0 4 2 2 0 0 0 0-4z": "more_vert",
  "M12 3a9 9 0 1 0 9 9c0-.46-.04-.92-.1-1.36a5.39 5.39 0 0 1-4.4 2.26 5.4 5.4 0 0 1-3.14-9.8c-.44-.06-.9-.1-1.36-.1z": "dark_mode",
  "M15.5 14h-.79l-.28-.27a6.5 6.5 0 1 0-.7.7l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0A4.5 4.5 0 1 1 14 9.5 4.5 4.5 0 0 1 9.5 14z": "search",
  "M7.4 9.2a.9.9 0 0 1 1.27 0L12 12.6l3.3-3.4a.9.9 0 1 1 1.28 1.27l-3.95 4.06a.9.9 0 0 1-1.27 0L7.4 10.47a.9.9 0 0 1 0-1.27z": "expand_more",
  "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z": "arrow_back",
  "M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4z": "photo_camera",
  "M12 5.5v13M5.5 12h13": "add",
  "M9 2 7.17 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-3.17L15 2H9zm3 15a5 5 0 1 1 0-10 5 5 0 0 1 0 10z": "photo_camera",
  "M15 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm-9-2V7H4v3H1v2h3v3h2v-3h3v-2H6zm9 4c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z": "person_add",
  "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm-3 7a2 2 0 1 1 0 4 2 2 0 0 1 0-4zm6 0a2 2 0 1 1 0 4 2 2 0 0 1 0-4zm-6 8c-1.66 0-3-1.34-3-3h12c0 1.66-1.34 3-3 3H9z": "groups",
  "M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z": "download",
  "M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z": "close",
  "M18.4 4.2 12 10.6 5.6 4.2l-1.4 1.4L10.6 12l-6.4 6.4 1.4 1.4L12 13.4l6.4 6.4 1.4-1.4L13.4 12l6.4-6.4-1.4-1.4z": "close",
  "M12 8l-6 6 1.41 1.41L12 10.83l4.59 4.58L18 14z": "expand_less",
  "M12 16l6-6-1.41-1.41L12 13.17 7.41 8.59 6 10z": "expand_more",
  "M12 16.5 5.5 10l1.4-1.4L12 13.7l5.1-5.1L18.5 10 12 16.5z": "keyboard_arrow_down",

  // sticker/emoji picker tabs
  "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 11h-5v-2h3V6h2v7z": "schedule",
  "M12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z": "star",
  "M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z": "favorite",
  "M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z": "add",
  "M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6V5z": "add",
  "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm-3.5 7a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm7 0a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM12 18c-2.33 0-4.31-1.46-5.11-3.5h10.22C16.31 16.54 14.33 18 1": "mood",
  "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 18a8 8 0 1 1 0-16 8 8 0 0 1 0 16zm-3.5-9a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3zm7 0a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3zM12 17.5c2.33 0 4.31-1.46": "mood",
  "M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h9l7-7V5a2 2 0 0 0-2-2zm-1 11h-4v4h-2v-4H8v-2h4V8h2v4h4v2z": "sticker",

  // composer
  "M16.5 6v11.5a4 4 0 0 1-8 0V5a2.5 2.5 0 0 1 5 0v10.5a1 1 0 0 1-2 0V6H10v9.5a2.5 2.5 0 0 0 5 0V5a4 4 0 0 0-8 0v12.5a5.5 5.5 0 0 0 11 0V6h-1.5z": "attach_file",
  "M1.101 21.757 23.8 12.028 1.101 2.3l.011 7.912 13.623 1.816-13.623 1.817-.011 7.912z": "send",
  "M2.01 21 23 12 2.01 3 2 10l15 2-15 2z": "send",

  // message menu
  "M16 1H4a2 2 0 0 0-2 2v14h2V3h12V1zm3 4H8a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2zm0 16H8V7h11v14z": "content_copy",
  "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z": "edit",
  "M3 17.25V21h3.75L17.8 9.94l-3.75-3.75L3 17.25zM20.7 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z": "edit",
  "M12 2l1.9 4.6L18.5 8.5 13.9 10.4 12 15l-1.9-4.6L5.5 8.5l4.6-1.9L12 2zm6 12l.9 2.1L21 17l-2.1.9L18 20l-.9-2.1L15 17l2.1-.9L18 14z": "auto_awesome",
  "M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zm0 16H5V5h14v14zm-8-2l-4-4 1.4-1.4L11 14.2l4.6-4.6L17 11l-6 6z": "check_box",
  "M14 4l-1-2H5v20h2v-8h5l1 2h6V4h-5z": "flag",
  "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z": "info",

  // media viewer
  "M8 5v14l11-7z": "play_arrow",
  "M6 5h4v14H6zM14 5h4v14h-4z": "pause",
  "M3 9v6h4l5 5V4L7 9H3zm13.5 3A4.5 4.5 0 0 0 14 7.97v8.05A4.47 4.47 0 0 0 16.5 12zM14 3.23v2.06a7 7 0 0 1 0 13.42v2.06a9 9 0 0 0 0-17.54z": "volume_up",
  "M3 9v6h4l5 5V4L7 9H3zm13.5 3a4.5 4.5 0 0 0-2.5-4v8a4.5 4.5 0 0 0 2.5-4z": "volume_up",
  "M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z": "fullscreen",
  "M5 16h3v3h2v-5H5v2zm3-8H5v2h5V5H8v3zm6 11h2v-3h3v-2h-5v5zm2-11V5h-2v5h5V8h-3z": "fullscreen_exit",
  "M15.2 4.2a0.95 0.95 0 0 0-1.35 0l-7 7a0.95 0.95 0 0 0 0 1.35l7 7a0.95 0.95 0 1 0 1.35-1.35L9 12l6.2-6.2a0.95 0.95 0 0 0 0-1.35z": "chevron_left",
  "M8.8 4.2a0.95 0.95 0 0 1 1.35 0l7 7a0.95 0.95 0 0 1 0 1.35l-7 7a0.95 0.95 0 1 1-1.35-1.35L15 12 8.8 5.8a0.95 0.95 0 0 1 0-1.35z": "chevron_right",
  "M12 12a5.6 5.6 0 1 0 0-11.2": "add_reaction",
  "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zM8.5 7A1.5 1.5 0 1 1 7 8.5 1.5 1.5 0 0 1 8.5 7zm7 0A1.5 1.5 0 1 1 14 8.5 1.5 1.5 0 0 1 15.5 7zM12 17.6a5.6 5.6 0 0 1-5.4-3.6h10.8a5.6 5.6 0 0 1-5.": "add_reaction",

  // audio player glyphs
  "m8 5.5 11 6.5-11 6.5z": "play_arrow",
  "M7 5h3.6v14H7zm6.4 0H17v14h-3.6z": "pause",

  // status viewer / composer
  "M9 4.5 7.5 12 15 19.5": "chevron_left",
  "M9 4.5 16.5 12 9 19.5": "chevron_right",
  "M15 4.5 7.5 12 15 19.5": "chevron_left",
  "M22 16V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2Zm-11-4 2.03 2.71L16 11l4 5H8l3-4Z": "photo_library",
  "M12 3a9 9 0 1 0 0 18c1.1 0 2-.9 2-2 0-.5-.2-.9-.5-1.3-.3-.3-.5-.7-.5-1.2 0-1.1.9-2 2-2h1.5A4.5 4.5 0 0 0 21 10c0-3.9-4-7-9-7Z": "palette",
  "M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.29-.7.29-.28 0-.53-.11-.71-.29L.29 13.08a.996.996 0 0 1 0-1.41C3.34 8.77 7.46 7 12 7s8.66 1.77 11.": "call_end",
  "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25Zm17.71-10.21a.996.996 0 0 0 0-1.41": "edit",
  "M8.4 14.6a4.6 4.6 0 0 0 7.2 0": "mood",
  "M3 9v6h4l5 5V4L7 9H3z": "volume_off",
};

const dir = "apps/desktop/ui";
const files = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith(".js") || f.endsWith(".html"))
  .filter((f) => f !== "emoji-data.js");

const keys = Object.keys(MAP).sort((a, b) => b.length - a.length);
const svgRe = /<svg\b[^>]*>[\s\S]*?<\/svg>/g;
const pathRe = /<path\b[^>]*\bd="([^"]+)"/g;

let replaced = 0;
const unresolved = [];

for (const file of files) {
  const p = path.join(dir, file);
  let text = fs.readFileSync(p, "utf8");
  let touched = false;
  text = text.replace(svgRe, (svg) => {
    const open = svg.slice(0, svg.indexOf(">") + 1);
    const ds = [...svg.matchAll(pathRe)].map((m) => m[1]);
    if (!ds.length) return svg;
    const first = ds[0];
    const key = keys.find((k) => first.startsWith(k));
    if (!key) {
      unresolved.push(`${file}: ${first.slice(0, 60)}`);
      return svg;
    }
    const name = MAP[key];
    const body = paths[name];
    if (!body) {
      unresolved.push(`${file}: missing material ${name}`);
      return svg;
    }
    replaced += 1;
    touched = true;
    // Material artwork is fill-based: drop stroke/fill overrides so the
    // paths render, keep class/id/aria/viewBox.
    const attrs = open
      .replace(/\sfill="none"/g, "")
      .replace(/\sstroke(?:-width|-linecap|-linejoin)?="[^"]*"/g, "")
      .replace(/\sclass="svg-icon\b/g, ' class="wa-icon');
    return `${attrs}${body}</svg>`;
  });
  if (touched) fs.writeFileSync(p, text);
}

console.log(`replaced ${replaced} svgs`);
if (unresolved.length) {
  console.log("UNRESOLVED:");
  for (const u of [...new Set(unresolved)]) console.log("  " + u);
}
