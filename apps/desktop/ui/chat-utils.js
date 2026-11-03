/* Pure helpers shared by the UI and the Node test harness. */
(function (global) {
  function isPlaceholderName(jid, name) {
    return name === jid.split("@")[0];
  }

  // Cleared history and "delete for me" messages are kept but hidden until the
  // eye toggle is on.
  function isMessageVisible(message, showDeleted) {
    if (showDeleted) return true;
    if (message.cleared) return false;
    if (message.deletedForMe) return false;
    return true;
  }

  const PREVIEW_LABELS = {
    image: "Photo",
    video: "Video",
    audio: "Audio",
    document: "Document",
    sticker: "Sticker",
    view_once: "View once message",
  };

  // A chat's mute expiry (`muteUntil`, Unix ms; `muteForever` for "Always")
  // decides whether the row shows WhatsApp's crossed-out bell.
  function chatIsMuted(chat) {
    return Boolean(
      chat && (chat.muteForever || (chat.muteUntil || 0) > Date.now()),
    );
  }

  // The chat list preview must never leak a message that is hidden right now:
  // walk back to the newest message the eye toggle would actually show.
  function lastVisibleMessage(chat, showDeleted) {
    const messages = chat.messages || [];
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (isMessageVisible(messages[i], showDeleted)) return messages[i];
    }
    if (chat.last && isMessageVisible(chat.last, showDeleted)) return chat.last;
    return null;
  }

  // Ctrl+V with an image on the clipboard: pick the first image item.
  function pickClipboardImage(items) {
    for (const item of items || []) {
      if (!item || item.kind === "string") continue;
      if (typeof item.type === "string" && item.type.startsWith("image/")) {
        const file = typeof item.getAsFile === "function" ? item.getAsFile() : null;
        if (file) return file;
      }
    }
    return null;
  }

  // "6285…:53@s.whatsapp.net" -> "6285…"
  function phoneFromJid(jid) {
    if (!jid) return "";
    const user = String(jid).split("@")[0];
    return user.split(":")[0];
  }

  // Phone-number user part -> WhatsApp-style "+62 896-3384-66984".
  // Country codes that are two digits; one-digit codes are 1 and 7 and
  // everything else falls back to three digits (+254, +998, ...).
  const CC2 = new Set([
    "20", "27", "30", "31", "32", "33", "34", "36", "39", "40", "41", "43",
    "44", "45", "46", "47", "48", "49", "51", "52", "53", "54", "55", "56",
    "57", "58", "60", "61", "62", "63", "64", "65", "66", "81", "82", "84",
    "86", "90", "91", "92", "93", "94", "95", "98",
  ]);

  function groupPhone(rest) {
    const groups = [];
    for (let i = 0; i < rest.length; i += 4) groups.push(rest.slice(i, i + 4));
    return groups.filter(Boolean).join("-");
  }

  // Phone-number user part -> a readable international number. Handles +1,
  // +62 and +86 grouping, and keeps other country codes intact.
  function formatPhone(phone) {
    const digits = String(phone || "").replace(/\D/g, "");
    if (!digits) return "";

    let ccLength = 3;
    if (digits.startsWith("1") || digits.startsWith("7")) ccLength = 1;
    else if (CC2.has(digits.slice(0, 2))) ccLength = 2;

    const cc = digits.slice(0, ccLength);
    const rest = digits.slice(ccLength);
    if (!rest) return `+${cc}`;

    if (cc === "1") {
      return `+1 ${rest.slice(0, 3)}-${rest.slice(3, 6)}-${rest.slice(6)}`;
    }
    if (cc === "62" || cc === "86") {
      return `+${cc} ${[rest.slice(0, 3), rest.slice(3, 7), rest.slice(7)]
        .filter(Boolean)
        .join("-")}`;
    }
    return `+${cc} ${groupPhone(rest)}`;
  }

  // WhatsApp gives each group participant a colour derived from their JID.
  // The set is the official `--WDS-components-profile-photo-content-*` palette.
  const SENDER_COLORS = [
    "#DBA685", // brown
    "#53A6FD", // cobalt
    "#BDBDBD", // gray
    "#25D366", // green
    "#FC9775", // orange
    "#FF72A1", // pink
    "#A791FF", // purple
    "#FB5061", // red
    "#53BDEB", // sky blue
    "#42C7B8", // teal
    "#FFD279", // yellow
  ];

  function senderColor(jid) {
    const text = String(jid || "");
    let hash = 0;
    for (let i = 0; i < text.length; i += 1) {
      hash = (hash * 31 + text.charCodeAt(i)) | 0;
    }
    return SENDER_COLORS[Math.abs(hash) % SENDER_COLORS.length];
  }

  // First web link in a message body, without trailing punctuation.
  const URL_PATTERN = /https?:\/\/[^\s<>"']+/i;

  function firstUrl(text) {
    if (!text) return null;
    const match = String(text).match(URL_PATTERN);
    if (!match) return null;
    return match[0].replace(/[),.;!?]+$/, "");
  }

  // "/ai explain this" -> "explain this"; anything else -> null.
  function aiPrompt(text) {
    if (!text) return null;
    const trimmed = String(text).trim();
    const lower = trimmed.toLowerCase();
    if (lower !== "/ai" && !lower.startsWith("/ai ")) return null;
    const rest = trimmed.slice(3).trim();
    return rest || null;
  }

  const api = {
    isPlaceholderName,

    pickClipboardImage,

    isMessageVisible,

    chatIsMuted,
    lastVisibleMessage,
    phoneFromJid,

    formatPhone,
    senderColor,
    firstUrl,

    aiPrompt,
  };

  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  } else {
    global.WAChatUtils = api;
  }
})(typeof globalThis !== "undefined" ? globalThis : this);
