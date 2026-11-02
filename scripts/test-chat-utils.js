const assert = require("assert");
// The UI loads icons.js before chat-utils.js (classic scripts share the
// global scope); mirror that here — under Node the helper must land on
// globalThis for chat-utils' free `waIcon` reference to resolve.
globalThis.waIcon = require("../apps/desktop/ui/icons.js").waIcon;
const {
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
  lastVisibleMessage,
  phoneFromJid,
  chatSubtitle,
  formatPhone,
  senderColor,
  firstUrl,
  siteOf,
  aiPrompt,
} = require("../apps/desktop/ui/chat-utils.js");

// A LID chat and a PN chat for the same person must collapse into one bar,
// keeping every message once.
const chats = new Map();
chats.set("111111111111111@lid", {
  jid: "111111111111111@lid",
  name: "Alice",
  isGroup: false,
  messages: [{ id: "a", timestamp: 1000 }],
  unread: 2,
  last: { id: "a", timestamp: 1000 },
  lastTs: 1000,
});
chats.set("6281234567890@s.whatsapp.net", {
  jid: "6281234567890@s.whatsapp.net",
  name: "Alice",
  isGroup: false,
  messages: [{ id: "b", timestamp: 2000 }],
  unread: 0,
  last: { id: "b", timestamp: 2000 },
  lastTs: 2000,
});

const { changed, remap } = mergeChats(chats);
assert.strictEqual(changed, true, "duplicates are merged");
assert.strictEqual(chats.size, 1, "one chat remains");
assert.strictEqual(remap.size, 1, "the dropped chat is remapped");

const survivor = [...chats.values()][0];
assert.strictEqual(survivor.messages.length, 2, "messages from both chats are kept");
assert.deepStrictEqual(
  survivor.messages.map((message) => message.id),
  ["a", "b"],
  "messages are ordered by timestamp",
);
assert.strictEqual(survivor.unread, 2, "unread counters add up");
assert.strictEqual(survivor.jid, "6281234567890@s.whatsapp.net", "the richer chat wins");

// Different people with different names are untouched.
const other = new Map();
other.set("1@s.whatsapp.net", { jid: "1@s.whatsapp.net", name: "A", isGroup: false, messages: [], unread: 0 });
other.set("2@s.whatsapp.net", { jid: "2@s.whatsapp.net", name: "B", isGroup: false, messages: [], unread: 0 });
assert.strictEqual(mergeChats(other).changed, false, "distinct names are not merged");

// Numeric placeholders never merge.
assert.strictEqual(isPlaceholderName("6281@s.whatsapp.net", "6281"), true);

// Memory budget: per-chat cap, then a global budget trimmed from the oldest
// conversation while the open chat is protected.
const heavy = new Map();
heavy.set("a", {
  jid: "a",
  messages: Array.from({ length: 10 }, (_, i) => ({ id: `a${i}`, timestamp: i })),
  lastTs: 10,
});
heavy.set("b", {
  jid: "b",
  messages: Array.from({ length: 10 }, (_, i) => ({ id: `b${i}`, timestamp: i })),
  lastTs: 20,
});

const dropped = trimMessages(heavy, { perChat: 6, budget: 8, keepJid: "b" });
const total = [...heavy.values()].reduce((sum, chat) => sum + chat.messages.length, 0);
assert.strictEqual(dropped, 4, "the over-budget messages are dropped");
assert.strictEqual(total, 8, "the global budget is respected");
assert.strictEqual(heavy.get("a").messages.length, 2, "the stale chat is trimmed first");
assert.strictEqual(heavy.get("b").messages.length, 6, "the open chat is only trimmed per-chat");
assert.strictEqual(
  heavy.get("a").messages[1].id,
  "a9",
  "the newest messages survive",
);

// Ctrl+V image detection: text-only clipboards are ignored, the first image
// item wins, and sizes render like WhatsApp.
const imageFile = { name: "clip.png", size: 2048, type: "image/png" };
assert.strictEqual(
  pickClipboardImage([{ kind: "string", type: "text/plain", getAsFile: () => null }]),
  null,
  "text-only clipboard does not open a draft",
);
assert.strictEqual(
  pickClipboardImage([
    { kind: "string", type: "text/plain", getAsFile: () => null },
    { kind: "file", type: "image/png", getAsFile: () => imageFile },
  ]),
  imageFile,
  "image item becomes the draft file",
);
assert.strictEqual(fileSizeLabel(2048), "2 KB");
assert.strictEqual(fileSizeLabel(3 * 1024 * 1024), "3.0 MB");

// Eye toggle: deleted chats and cleared history are kept but hidden by default.
const deletedChat = { jid: "1@s.whatsapp.net", name: "Gone", deleted: true, messages: [] };
assert.strictEqual(isChatVisible(deletedChat, false), false, "deleted chat is hidden");
assert.strictEqual(isChatVisible(deletedChat, true), true, "the eye reveals the deleted chat");
assert.strictEqual(isChatVisible({ jid: "2@s.whatsapp.net", messages: [] }, false), true);

const clearedMessage = { id: "m1", cleared: true, text: "old" };
assert.strictEqual(isMessageVisible(clearedMessage, false), false, "cleared message is hidden");
assert.strictEqual(isMessageVisible(clearedMessage, true), true, "the eye reveals cleared history");

const deletedMessage = { id: "m2", deleted: true, text: "secret text" };
assert.strictEqual(
  deletedMessageText(deletedMessage, false),
  "",
  "deleted text is not revealed by default",
);
assert.strictEqual(
  deletedMessageText(deletedMessage, true),
  "secret text",
  "the eye reveals the original text",
);
assert.strictEqual(deletedMessageText({ id: "m3", text: "hi" }, true), null, "normal messages untouched");

// "Delete for me": hidden entirely until the eye is on, then the original text.
const deletedForMe = { id: "m4", deleted: true, deletedForMe: true, text: "kept locally" };
assert.strictEqual(isMessageVisible(deletedForMe, false), false, "delete-for-me removes the bubble");
assert.strictEqual(isMessageVisible(deletedForMe, true), true, "the eye brings it back");
assert.strictEqual(deletedMessageText(deletedForMe, true), "kept locally");

// Chat-list previews follow the official rules: a direct chat shows only the
// message text ("Pak Gunawan: Siap gpp kok" becomes "Siap gpp kok"), and the
// delivery tick is separate markup (`previewTick`) so it can go blue on read.
assert.strictEqual(
  previewOf({ from_me: true, kind: "text", text: "test" }, false, "Alice"),
  "test",
);
assert.strictEqual(
  previewTick({ from_me: true, kind: "text" }, false) ===
    `<span class="tick preview-tick">${waIcon("check")}</span>`,
  true,
  "sent messages get a single grey tick",
);
assert.strictEqual(
  previewTick({ from_me: true, kind: "text", status: "delivered" }, false) ===
    `<span class="tick preview-tick">${waIcon("done_all")}</span>`,
  true,
  "delivered messages get the grey double tick",
);
assert.strictEqual(
  previewTick({ from_me: true, kind: "text", status: "read" }, false) ===
    `<span class="tick preview-tick read">${waIcon("done_all")}</span>`,
  true,
  "read messages get the blue double tick",
);
assert.strictEqual(
  previewTick({ from_me: true, kind: "text" }, true),
  "",
  "groups credit You: instead of a tick",
);
assert.strictEqual(previewTick(null, false), "");
assert.strictEqual(
  previewOf(
    { from_me: false, kind: "text", text: "Siap gpp kok 👌", sender_name: "Pak Gunawan" },
    false,
    "Pak Gunawan",
  ),
  "Siap gpp kok 👌",
);
assert.strictEqual(
  previewOf({ from_me: false, kind: "text", text: "hi", sender_name: "Bob" }, true, "Team Chat"),
  "~ Bob: hi",
  "an unsaved group member keeps WhatsApp Web's ~ marker in the preview",
);
assert.strictEqual(
  previewOf({ from_me: false, kind: "image", sender_name: "Alice" }, true, "Team Chat"),
  "~ Alice: Photo",
);
assert.strictEqual(
  previewOf({ from_me: true, kind: "text", text: "good job" }, true, "Team Chat"),
  "You: good job",
);

// Chat-list previews never leak a deleted or hidden message.
assert.strictEqual(
  previewOf({ id: "m5", from_me: true, kind: "text", text: "secret", deleted: true }, false, "Alice"),
  "This message was deleted",
  "a deleted message is masked in the preview",
);
assert.strictEqual(
  previewOf(
    { id: "m5", from_me: true, kind: "text", text: "secret", deleted: true },
    false,
    "Alice",
    true,
  ),
  "secret",
  "the eye reveals the original preview text",
);

const hiddenChat = {
  name: "Alice",
  messages: [
    { id: "v1", from_me: false, kind: "text", text: "visible", timestamp: 1 },
    {
      id: "v2",
      from_me: true,
      kind: "text",
      text: "delete for me",
      timestamp: 2,
      deleted: true,
      deletedForMe: true,
    },
  ],
  last: {
    id: "v2",
    from_me: true,
    kind: "text",
    text: "delete for me",
    timestamp: 2,
    deleted: true,
    deletedForMe: true,
  },
};
assert.strictEqual(
  lastVisibleMessage(hiddenChat, false).id,
  "v1",
  "the preview falls back past a hidden delete-for-me message",
);
assert.strictEqual(lastVisibleMessage(hiddenChat, true).id, "v2", "the eye reveals it again");

// Settings screen shows a clean phone number from the account JID.
assert.strictEqual(phoneFromJid("6280987654321:53@s.whatsapp.net"), "6280987654321");
assert.strictEqual(phoneFromJid("6280987654321@s.whatsapp.net"), "6280987654321");
assert.strictEqual(phoneFromJid(""), "");

// Link previews pick the first web URL and drop trailing punctuation.
assert.strictEqual(firstUrl("look https://www.instagram.com/p/abc/?x=1 nice"), "https://www.instagram.com/p/abc/?x=1");
assert.strictEqual(firstUrl("(https://youtu.be/abc)."), "https://youtu.be/abc");
assert.strictEqual(firstUrl("no links here"), null);
assert.strictEqual(firstUrl(""), null);
assert.strictEqual(siteOf("https://www.instagram.com/p/abc/"), "www.instagram.com");
assert.strictEqual(siteOf("not a url"), "");

// AI trigger: only a leading /ai opens a prompt.
assert.strictEqual(aiPrompt("/ai hello there"), "hello there");
assert.strictEqual(aiPrompt("/AI   hello "), "hello");
assert.strictEqual(aiPrompt("/ai"), null);
assert.strictEqual(aiPrompt("hey /ai hello"), null);
assert.strictEqual(aiPrompt(""), null);

// Chat header subtitle, like WhatsApp Web.
assert.strictEqual(
  chatSubtitle("6280987654321:53@s.whatsapp.net", false, null, null),
  "+62 809-8765-4321",
);
assert.strictEqual(
  chatSubtitle(
    "111111111111111:53@lid",
    false,
    "6280987654321:53@s.whatsapp.net",
    "111111111111111:53@lid",
  ),
  "Message yourself",
);
assert.strictEqual(chatSubtitle("1203630@g.us", true, null, null), "Group chat");
assert.strictEqual(chatSubtitle("111111111111111@lid", false, null, null), "");
assert.strictEqual(
  chatSubtitle("111111111111111@lid", false, null, null, "6281234567890"),
  "+62 812-3456-7890",
);
assert.strictEqual(chatSubtitle("", false, null, null), "");

// Phone formatting for both local and international senders.
assert.strictEqual(formatPhone("+62 809-8765-4321"), "+62 809-8765-4321");
assert.strictEqual(formatPhone("6280987654321"), "+62 809-8765-4321");
assert.strictEqual(formatPhone("14155552671"), "+1 415-555-2671");
assert.strictEqual(formatPhone("8613800138000"), "+86 138-0013-8000");
assert.strictEqual(formatPhone("447911123456"), "+44 7911-1234-56");
assert.strictEqual(formatPhone(""), "");

console.log("chat-utils: all assertions passed");
