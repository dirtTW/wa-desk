/* Pure helpers shared by the UI and the Node test harness. */
(function (global) {
  function isPlaceholderName(jid, name) {
    return name === jid.split("@")[0];
  }

  // A deleted chat stays in memory; the eye toggle decides whether it shows.
  function isChatVisible(chat, showDeleted) {
    return Boolean(showDeleted) || !chat.deleted;
  }

  // Cleared history and "delete for me" messages are kept but hidden until the
  // eye toggle is on.
  function isMessageVisible(message, showDeleted) {
    if (showDeleted) return true;
    if (message.cleared) return false;
    if (message.deletedForMe) return false;
    return true;
  }

  // `null` when the message was not deleted at all, `""` when deleted but
  // hidden, or the original text when the eye toggle reveals it.
  function deletedMessageText(message, showDeleted) {
    if (!message.deleted) return null;
    return showDeleted && message.text ? message.text : "";
  }

  const PREVIEW_LABELS = {
    image: "Photo",
    video: "Video",
    audio: "Audio",
    document: "Document",
    sticker: "Sticker",
    view_once: "View once message",
  };

  function previewOf(message, isGroup, chatName, showDeleted) {
    let text;
    if (message.deleted) {
      text = showDeleted && message.text ? message.text : "This message was deleted";
    } else if (message.kind === "document") {
      // WhatsApp lists the file name, not a generic "Document".
      text = message.document?.name || message.text || "Document";
    } else if (message.kind !== "text" && PREVIEW_LABELS[message.kind]) {
      text = message.text
        ? `${PREVIEW_LABELS[message.kind]}: ${message.text}`
        : PREVIEW_LABELS[message.kind];
    } else {
      text = message.text || "Unsupported message";
    }

    let who = "";
    if (message.from_me) {
      // Official rules for the preview text: groups credit "You:"; a direct
      // chat shows only the message — its delivery tick is drawn separately
      // by `previewTick` so it can pick up the read colour.
      who = isGroup ? "You: " : "";
    } else if (isGroup) {
      // Unsaved members get their push name with a "~" (WhatsApp Web), and the
      // phone number is the last resort instead of the literal "Unknown".
      const saved =
        typeof global.contactNameFor === "function"
          ? global.contactNameFor(message.sender)
          : "";
      const push = message.sender_name || "";
      const label =
        saved ||
        (push ? `~ ${push}` : "") ||
        formatPhone(message.sender_phone || "") ||
        "Unknown";
      who = `${label}: `;
    }

    return `${who}${text}`;
  }

  // The sidebar's delivery marker for one of our own messages in a direct
  // chat: grey single tick until a receipt lands, grey double tick once
  // delivered, blue double tick when it was read ("played" counts too).
  // Missing receipts just stay grey — WhatsApp only sends one when the other
  // side has read confirmations enabled.
  function previewTick(message, isGroup) {
    if (!message || !message.from_me || isGroup) return "";
    const read = message.status === "read" || message.status === "played";
    const delivered = message.status === "delivered";
    const glyph = read || delivered ? waIcon("done_all") : waIcon("check");
    return `<span class="tick preview-tick${read ? " read" : ""}">${glyph}</span>`;
  }

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

  function fileSizeLabel(bytes) {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${bytes} B`;
  }

  // Keep memory bounded: per-chat cap plus a global budget. Chats with the
  // oldest activity are trimmed first, and the open chat is never touched.
  function trimMessages(chats, options) {
    const perChat = options.perChat;
    const budget = options.budget;
    const keepJid = options.keepJid;

    let total = 0;
    for (const chat of chats.values()) {
      if (chat.messages.length > perChat) {
        chat.messages.splice(0, chat.messages.length - perChat);
      }
      total += chat.messages.length;
    }

    if (total <= budget) return 0;

    const order = [...chats.values()]
      .filter((chat) => chat.jid !== keepJid)
      .sort((a, b) => (a.lastTs || 0) - (b.lastTs || 0));

    let dropped = 0;
    for (const chat of order) {
      if (total <= budget) break;
      const overflow = Math.min(chat.messages.length, total - budget);
      if (overflow <= 0) continue;
      chat.messages.splice(0, overflow);
      total -= overflow;
      dropped += overflow;
    }

    return dropped;
  }

  // Two conversations for the same person (PN vs LID, or a re-sync) end up
  // with the same display name. Keep the richer one and fold the other in.
  function mergeChats(chats) {
    const byName = new Map();
    const remap = new Map();
    let changed = false;

    for (const chat of [...chats.values()]) {
      if (chat.isGroup) continue;
      // Channels are broadcast-only and must never be folded into a DM that
      // happens to share a display name.
      if (String(chat.jid).endsWith("@newsletter")) continue;
      if (isPlaceholderName(chat.jid, chat.name)) continue;
      const key = String(chat.name || "").trim().toLowerCase();
      if (!key) continue;

      const existing = byName.get(key);
      if (!existing) {
        byName.set(key, chat);
        continue;
      }

      const target =
        existing.messages.length === chat.messages.length
          ? existing.jid.endsWith("@lid") && !chat.jid.endsWith("@lid")
            ? chat
            : existing
          : existing.messages.length > chat.messages.length
            ? existing
            : chat;
      const source = target === existing ? chat : existing;

      for (const message of source.messages) {
        if (!target.messages.some((candidate) => candidate.id === message.id)) {
          target.messages.push(message);
        }
      }
      target.messages.sort((a, b) => a.timestamp - b.timestamp);
      target.unread += source.unread;
      // A surviving copy means the conversation is not deleted.
      target.deleted = Boolean(target.deleted && source.deleted);
      target.cleared = Boolean(target.cleared && source.cleared);
      if ((source.last?.timestamp || 0) > (target.last?.timestamp || 0)) {
        target.last = source.last;
        target.lastTs = source.lastTs;
      }

      chats.delete(source.jid);
      remap.set(source.jid, target.jid);
      byName.set(key, target);
      changed = true;
    }

    return { changed, remap };
  }

  // "6285…:53@s.whatsapp.net" -> "6285…"
  function phoneFromJid(jid) {
    if (!jid) return "";
    const user = String(jid).split("@")[0];
    return user.split(":")[0];
  }

  // Chat header subtitle, like WhatsApp Web: a phone number or a group. A LID
  // only becomes a number once the protocol layer resolves its PN.
  function chatSubtitle(jid, isGroup, meJid, meLid, phone) {
    if (isGroup) return "Group chat";
    if (jid && (jid === meJid || jid === meLid)) return "Message yourself";
    let raw = phone || "";
    if (!raw && jid && !String(jid).endsWith("@lid")) raw = phoneFromJid(jid);
    return raw ? formatPhone(raw) : "";
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

  // "https://www.instagram.com/p/..." -> "www.instagram.com"
  function siteOf(url) {
    try {
      return new URL(String(url)).hostname;
    } catch {
      return "";
    }
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
    mergeChats,
    trimMessages,
    pickClipboardImage,
    fileSizeLabel,
    isChatVisible,
    isMessageVisible,
    deletedMessageText,
    previewOf,
    previewTick,
    chatIsMuted,
    lastVisibleMessage,
    phoneFromJid,
    chatSubtitle,
    formatPhone,
    senderColor,
    firstUrl,
    siteOf,
    aiPrompt,
  };

  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  } else {
    global.WAChatUtils = api;
  }
})(typeof globalThis !== "undefined" ? globalThis : this);
