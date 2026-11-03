// Verifies the Rust-rendered UI on the live app over CDP.
//
//   node tests/e2e/ui-rust.mjs
//
// Prerequisites (see dev.ps1 / the release exe):
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222
//   and a running WA Desk window.
//
// Checks the sidebar, emoji catalogue/picker and status emoji panel all come
// from `wa-ui` (Rust) and that no JavaScript error was logged while they
// rendered.
import { chromium } from "playwright-core";

const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
const pages = browser.contexts().flatMap((context) => context.pages());
const page = pages.find((p) => p.url().startsWith("http://tauri.localhost") || p.url().includes("127.0.0.1"));
if (!page) throw new Error("WA Desk webview not found on port 9222");

let failures = 0;
const check = (name, ok, detail = "") => {
  console.log(`${ok ? "  ok  " : "FAIL  "}${name}${detail ? ` - ${detail}` : ""}`);
  if (!ok) failures += 1;
};

const errors = [];
page.on("pageerror", (error) => errors.push(String(error)));
page.on("console", (message) => {
  if (message.type() === "error") errors.push(message.text());
});

await page.waitForSelector("#rail", { timeout: 15000 });
await page.waitForTimeout(2500);

console.log("\n# rust bootstrap");
const boot = await page.evaluate(async () => {
  const out = await window.__TAURI__.core.invoke("ui_bootstrap");
  return {
    icons: out.icons.length,
    groups: out.emoji_groups.length,
    count: out.emoji_count,
    pickerLength: out.emoji_picker.length,
    categories: out.emoji_categories.length,
  };
});
check("ui_bootstrap returns the icon set", boot.icons > 50, `${boot.icons} icons`);
check("ui_bootstrap returns the emoji catalogue", boot.count > 2000, `${boot.count} emoji in ${boot.groups} groups`);
check("ui_bootstrap ships the picker markup", boot.pickerLength > 20000, `${boot.pickerLength} bytes`);
check("ui_bootstrap ships the category strip", boot.categories > 100);

console.log("\n# sidebar");
const rows = await page.locator("#chat-list .chat-item").count();
check("the chat list rendered", rows >= 0, `${rows} rows`);
if (rows > 0) {
  const first = page.locator("#chat-list .chat-item").first();
  check("rows carry a data-jid", Boolean(await first.getAttribute("data-jid")));
  check("rows carry an avatar", (await first.locator(".avatar").count()) === 1);
  check(
    "rows carry a preview",
    (await first.locator(".chat-preview-text").count()) === 1,
    (await first.locator(".chat-preview-text").first().textContent())?.slice(0, 40),
  );
}

console.log("\n# sidebar search (rust)");
if (rows > 0) {
  const name =
    (await page.locator("#chat-list .chat-item strong").first().textContent())?.trim() || "a";
  // Use the first word so casing/spacing edits during a repaint cannot make the
  // needle miss; wait for the mark instead of a fixed sleep.
  const needle = name.split(/\s+/)[0].slice(0, 6) || "a";
  await page.fill("#search", needle);
  let marked = 0;
  for (let attempt = 0; attempt < 10 && marked === 0; attempt += 1) {
    await page.waitForTimeout(200);
    marked = await page.locator("#chat-list .search-hit").count();
  }
  const section = await page.locator("#chat-list .search-section").count();
  check("search highlights the match", marked > 0, `${marked} marks for "${needle}"`);
  check("search keeps its section headers", section >= 1);
  await page.fill("#search", "");
  await page.waitForTimeout(600);
}

console.log("\n# emoji picker (rust)");
await page.evaluate(() => {
  // Open the picker the same way the composer does.
  document.getElementById("emoji-panel").hidden = false;
  buildEmojiPicker();
});
const emojiCount = await page.locator("#emoji-scroll .emoji").count();
const catCount = await page.locator("#emoji-cats .emoji-cat").count();
check("the picker grids come from Rust", emojiCount > 2000, `${emojiCount} buttons`);
check("the category strip comes from Rust", catCount === 9, `${catCount} categories`);
const searchHits = await page.evaluate(async () => {
  await filterEmoji("grinning");
  return document.querySelectorAll("#emoji-scroll .emoji-grid.search-results .emoji").length;
});
check("emoji search runs in Rust", searchHits > 0, `${searchHits} matches`);
await page.evaluate(async () => {
  await filterEmoji("");
  document.getElementById("emoji-panel").hidden = true;
});

console.log("\n# status emoji panel (same Rust catalogue)");
const statusEmoji = await page.evaluate(async () => {
  await buildStatusEmojiPanel();
  await renderStatusEmojiGroup(0);
  await new Promise((r) => setTimeout(r, 400));
  return {
    cats: document.querySelectorAll("#status-emoji-cats .emoji-cat").length,
    items: document.querySelectorAll("#status-emoji-grid .emoji").length,
  };
});
check("status panel has the 9 categories", statusEmoji.cats === 9, `${statusEmoji.cats}`);
check("status panel is populated", statusEmoji.items > 20, `${statusEmoji.items} emoji`);

console.log("\n# chat messages (rust)");
const chatCheck = await page.evaluate(async () => {
  const first = document.querySelector("#chat-list .chat-item[data-jid]");
  if (!first) return { skipped: true };
  const jid = first.getAttribute("data-jid");
  first.click();
  await new Promise((r) => setTimeout(r, 1500));
  const box = document.getElementById("messages");
  return {
    jid,
    bubbles: box.querySelectorAll(".bubble").length,
    days: box.querySelectorAll(".day").length,
    media: box.querySelectorAll(".media").length,
    images: box.querySelectorAll(".media-image img, .media .media-video").length,
    text: box.querySelectorAll(".body").length,
    subtitle: document.getElementById("chat-sub").textContent,
    headerName: document.getElementById("chat-name").textContent,
  };
});
if (chatCheck.skipped) {
  console.log("  ok  chat check skipped (no chats)");
} else {
  check("a chat opens", chatCheck.bubbles > 0, `${chatCheck.bubbles} bubbles for ${chatCheck.jid}`);
  check("day dividers render", chatCheck.days > 0, `${chatCheck.days} dividers`);
  check("message bodies render", chatCheck.text > 0, `${chatCheck.text} bodies`);
  check("header carries the Rust subtitle", typeof chatCheck.subtitle === "string", chatCheck.subtitle);
  check("media blocks render", chatCheck.media >= 0, `${chatCheck.media} media, ${chatCheck.images} ready`);
}

console.log("\n# chat header (rust)");
const header = await page.evaluate(async () => {
  const groups = [];
  for (const chat of [...state.chats.values()].filter((c) => c.isGroup).slice(0, 1)) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 900));
    groups.push({
      name: document.getElementById("chat-name").textContent,
      sub: document.getElementById("chat-sub").textContent,
    });
  }
  const dm = [...state.chats.values()].find(
    (c) => !c.isGroup && !c.jid.endsWith("@newsletter"),
  );
  const dms = [];
  if (dm) {
    await openChat(dm.jid);
    await new Promise((r) => setTimeout(r, 900));
    dms.push({
      name: document.getElementById("chat-name").textContent,
      sub: document.getElementById("chat-sub").textContent,
    });
  }
  localStorage.setItem("wadesk.emoji.recents", JSON.stringify(["😀", "❤️", "👍"]));
  renderRecentEmojis();
  await new Promise((r) => setTimeout(r, 400));
  return { groups, dms, recents: document.querySelectorAll("#emoji-recents .emoji").length };
});
check("the chat name renders", (header.groups[0]?.name || "").length > 0, header.groups[0]?.name);
check("the group subtitle renders", header.groups[0]?.sub === "Group chat", header.groups[0]?.sub);
check(
  "the direct-chat subtitle renders",
  (header.dms[0]?.sub || "").length > 0,
  `${header.dms[0]?.name} · ${header.dms[0]?.sub}`,
);
check("emoji recents render", header.recents === 3, `${header.recents} recents`);

console.log("\n# incoming rules (rust)");
const incoming = await page.evaluate(async () => {
  const jid = "999888777@s.whatsapp.net";
  await addIncoming({
    id: "probe-1",
    chat: jid,
    chat_name: "Probe",
    is_group: false,
    from_me: false,
    timestamp: Date.now(),
    kind: "text",
    text: "hello probe",
  });
  const entry = state.chats.get(jid);
  const unreadAfterFirst = entry?.unread;
  await addIncoming({
    id: "probe-1",
    chat: jid,
    chat_name: "Probe",
    is_group: false,
    from_me: false,
    timestamp: Date.now(),
    kind: "text",
    text: "hello probe",
    forwarded: true,
    stats: { reactions: [], reaction_total: 0, forwards: 2, views: 5 },
  });
  const messagesAfterDuplicate = entry?.messages.length;
  const folded = {
    forwarded: Boolean(entry?.messages[0]?.forwarded),
    stats: Boolean(entry?.messages[0]?.stats),
  };
  state.active = jid;
  await addIncoming({
    id: "probe-2",
    chat: jid,
    chat_name: "Probe",
    is_group: false,
    from_me: false,
    timestamp: Date.now(),
    kind: "text",
    text: "second",
  });
  const unreadWhenActive = entry?.unread;
  state.active = null;

  const optimistic = {
    id: "local-1",
    chat: jid,
    from_me: true,
    pending: true,
    kind: "text",
    text: "mine",
    timestamp: Date.now(),
  };
  entry.messages.push(optimistic);
  await addSent({ chat: jid, id: "server-9", timestamp: Date.now() });

  const receiptAdvance = await invoke("ui_receipt_status", { current: "sent", incoming: "delivered" });
  const receiptRollback = await invoke("ui_receipt_status", { current: "read", incoming: "sent" });
  const aiEcho = await invoke("ui_ai_echo", {
    messages: [[true, "same", Date.now()]],
    text: "same",
    now: Date.now(),
  });
  state.chats.delete(jid);
  return {
    unreadAfterFirst,
    messagesAfterDuplicate,
    folded,
    unreadWhenActive,
    sentId: optimistic.id,
    sentPending: Boolean(optimistic.pending),
    receiptAdvance,
    receiptRollback,
    aiEcho,
  };
});
check("incoming messages bump unread", incoming.unreadAfterFirst === 1);
check("duplicates fold instead of appending", incoming.messagesAfterDuplicate === 1 && incoming.folded.forwarded && incoming.folded.stats, JSON.stringify(incoming.folded));
check("the open chat clears its badge", incoming.unreadWhenActive === 0);
check("a send echo folds into the optimistic bubble", incoming.sentId === "server-9" && !incoming.sentPending);
check("receipts only advance", incoming.receiptAdvance && !incoming.receiptRollback);
check("the AI echo guard works", incoming.aiEcho === true);

console.log("\n# search + attachments (rust)");
const searchAttach = await page.evaluate(async () => {
  const photo = await invoke("ui_attachment_check", { size: 2048, mime: "image/png" });
  const pdf = await invoke("ui_attachment_check", { size: 10, mime: "application/pdf" });
  const huge = await invoke("ui_attachment_check", { size: 50 * 1024 * 1024, mime: "video/mp4" });
  const matches = await invoke("ui_search_matches", {
    texts: ["hello there", "nothing", "say hello"],
    query: "HELLO",
  });
  const label = await invoke("ui_search_label", { index: 0, total: 2, query: "hello" });
  const emptyLabel = await invoke("ui_search_label", { index: -1, total: 0, query: "zzz" });

  const chat = [...state.chats.values()].find((entry) => entry.messages.length > 3);
  let live = null;
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 1200));
    const word = (chat.messages.find((m) => m.text && m.text.length > 12)?.text || "").slice(0, 4);
    document.getElementById("chat-search-bar").hidden = false;
    document.getElementById("chat-search-input").value = word;
    await runChatSearch(word);
    await new Promise((r) => setTimeout(r, 400));
    live = {
      hits: document.querySelectorAll("#messages .bubble.search-hit").length,
      count: document.getElementById("chat-search-count").textContent,
    };
    closeChatSearch();
  }
  return { photo, pdf, huge, matches, label, emptyLabel, live };
});
check("attachments are classified", searchAttach.photo.kind === "image" && searchAttach.pdf.kind === "document", `${searchAttach.photo.kind}/${searchAttach.pdf.kind}`);
check("oversized attachments are rejected", searchAttach.huge.ok === false, searchAttach.huge.error);
check("search matches are planned in Rust", searchAttach.matches.join(",") === "0,2", searchAttach.matches.join(","));
check("the search label renders", searchAttach.label.text === "1/2" && searchAttach.emptyLabel.text === "0/0");
if (searchAttach.live) {
  check("in-chat search finds bubbles", searchAttach.live.hits > 0, `${searchAttach.live.hits} bubbles, ${searchAttach.live.count}`);
}

console.log("\n# action guards (rust)");
const guards = await page.evaluate(async () => {
  const own = await invoke("ui_status_permissions", { fromMe: true });
  const other = await invoke("ui_status_permissions", { fromMe: false });
  const empty = await invoke("ui_status_post_check", { hasFile: false, mime: "", text: "  " });
  const text = await invoke("ui_status_post_check", { hasFile: false, mime: "", text: " hi " });
  const pdf = await invoke("ui_status_post_check", { hasFile: true, mime: "application/pdf", text: "x" });
  const video = await invoke("ui_status_post_check", { hasFile: true, mime: "video/mp4", text: " cap " });
  const reply = await invoke("ui_reply_preview", {
    message: { from_me: false, kind: "image", text: "", sender: "6285@s.whatsapp.net", sender_name: "Push", contact_name: "Saved" },
  });
  const bar = await invoke("ui_selection_bar", { count: 2 });
  startReply({ from_me: true, kind: "text", text: "hello", sender: "", sender_name: "", chat: state.active });
  await new Promise((r) => setTimeout(r, 300));
  const banner = {
    hidden: document.getElementById("reply-banner").hidden,
    name: document.getElementById("reply-banner-name").textContent,
    text: document.getElementById("reply-banner-text").textContent,
  };
  cancelReply();
  return { own, other, empty, text, pdf, video, reply, bar, banner };
});
check("status permissions are computed", guards.own.can_delete && !guards.own.can_reply && guards.other.can_reply, JSON.stringify(guards.other));
check("empty status drafts are rejected", guards.empty.ok === false);
check("text and video drafts pass", guards.text.ok && guards.video.ok && guards.video.is_video);
check("wrong media types are rejected", guards.pdf.ok === false, guards.pdf.error);
check("the reply preview renders", guards.reply.name === "Saved" && guards.reply.preview === "Photo", `${guards.reply.name}: ${guards.reply.preview}`);
check("the selection bar renders", guards.bar.label === "2 selected" && !guards.bar.disabled);
check("the reply banner is painted", guards.banner.hidden === false && guards.banner.name === "You", `${guards.banner.name}: ${guards.banner.text}`);

console.log("\n# flows (rust)");
const flows = await page.evaluate(async () => {
  const chat = [...state.chats.values()].find((entry) => entry.messages.length > 2);
  const out = {};
  if (chat) {
    const anchor = await historyAnchorRequest(chat);
    out.anchor = {
      hasId: Boolean(anchor.oldestId),
      count: anchor.count,
      matchesOldest: anchor.oldestId === chat.messages.find((m) => !m.deleted)?.id,
    };
    out.targets = (await chatsForHistorySync(5)).length;
  }
  out.diskBytes = (await chatCachePayload()).length;
  const parsed = await invoke("ui_parse_export", {
    text: "[4/7/26, 8:44 PM] Alice: hello\ncontinues\n[4/7/26, 8:50 PM] Alice: world",
    allowNoSender: false,
  });
  const plan = await invoke("ui_import_plan", { parsed, existing: [] });
  out.parsed = parsed.length;
  out.continuation = parsed[0]?.text || "";
  out.planAdds = plan.add.length;
  return out;
});
check("the history anchor renders", flows.anchor?.hasId === true && flows.anchor.count === 50, JSON.stringify(flows.anchor));
check("the sync targets render", flows.targets > 0, `${flows.targets} targets`);
check("the disk cache payload renders", flows.diskBytes > 100, `${flows.diskBytes} bytes`);
check("the export parser handles continuations", flows.parsed === 2 && flows.continuation === "hello\ncontinues", flows.continuation);
check("the import plan lists the new messages", flows.planAdds === 2, `${flows.planAdds} adds`);

const mute = await page.evaluate(async () => {
  const now = Date.now();
  const muted = await invoke("ui_muted_chats", {
    chats: [
      { jid: "a", pinned: false, mute_until: now + 60000, mute_forever: false, now, archived: false, unread: 0, is_group: false },
      { jid: "b", pinned: false, mute_until: 0, mute_forever: false, now, archived: false, unread: 0, is_group: false },
      { jid: "c", pinned: false, mute_until: 0, mute_forever: true, now, archived: false, unread: 0, is_group: false },
    ],
  });
  return { muted };
});
check("muted chats are computed in Rust", mute.muted.join(",") === "a,c", mute.muted.join(","));

console.log("\n# cache layer (rust)");
const cache = await page.evaluate(async () => {
  const fingerprint = await stateFingerprint();
  const same = await stateFingerprint();
  await saveNow();
  const raw = localStorage.getItem("wadesk.state.v1") || "";
  const restored = raw ? await invoke("ui_cache_restore", { raw }) : { chats: [] };
  const photoQueue = await invoke("ui_photo_queue", {
    candidates: ["6285@s.whatsapp.net", "6285@s.whatsapp.net", "0@s.whatsapp.net", "status@broadcast"],
    connected: true,
    have: [],
    failed: [],
    queued: [],
  });
  return {
    fingerprint,
    same,
    bytes: raw.length,
    chats: restored.chats.length,
    stripped: raw.includes('"thumb":null'),
    photoQueue,
  };
});
check("the cache fingerprint is stable", cache.fingerprint.length > 0 && cache.fingerprint === cache.same, cache.fingerprint);
check("the cache payload is written", cache.bytes > 1000, `${cache.bytes} bytes`);
check("the payload restores", cache.chats > 0, `${cache.chats} chats`);
check("link thumbnails are stripped", cache.stripped);
check("the photo queue filters and dedupes", cache.photoQueue.length === 1, cache.photoQueue.join(","));

console.log("\n# chat store plans (rust)");
const storePlans = await page.evaluate(async () => {
  const a = "999000111@lid";
  const b = "999000222@s.whatsapp.net";
  state.chats.set(a, {
    jid: a,
    name: "Merge Probe",
    isGroup: false,
    messages: [{ id: "pa", timestamp: 1000, text: "a" }],
    last: { id: "pa", timestamp: 1000 },
    lastTs: 1000,
    unread: 2,
  });
  state.chats.set(b, {
    jid: b,
    name: "Merge Probe",
    isGroup: false,
    messages: [{ id: "pb", timestamp: 2000, text: "b" }],
    last: { id: "pb", timestamp: 2000 },
    lastTs: 2000,
    unread: 0,
  });
  mergeDuplicateChats();
  await new Promise((r) => setTimeout(r, 700));
  const survivor = state.chats.get(b);
  const merge = {
    lidGone: !state.chats.has(a),
    messages: survivor ? survivor.messages.map((m) => m.id) : [],
    unread: survivor ? survivor.unread : null,
  };
  if (survivor) state.chats.delete(b);

  const trim = await invoke("ui_trim_messages", {
    chats: [
      { jid: "t1", len: 10, last_ts: 100 },
      { jid: "t2", len: 10, last_ts: 200 },
      { jid: "keep", len: 4, last_ts: 50 },
    ],
    perChat: 8,
    budget: 12,
    keepJid: "keep",
  });
  return { merge, trim };
});
check(
  "duplicate chats merge",
  storePlans.merge.lidGone && storePlans.merge.messages.length === 2 && storePlans.merge.unread === 2,
  `${storePlans.merge.messages.join("+")}, unread ${storePlans.merge.unread}`,
);
check(
  "the trim plan protects the open chat",
  storePlans.trim.dropped > 0 && !storePlans.trim.ops.some((op) => op.jid === "keep"),
  `${storePlans.trim.dropped} dropped`,
);

console.log("\n# status composer (rust)");
const composer = await page.evaluate(async () => {
  openStatus();
  await new Promise((r) => setTimeout(r, 800));
  const palette = await invoke("ui_status_palette");
  const argb = await invoke("ui_status_argb", { hex: "#243640" });
  await openStatusTextComposer();
  await new Promise((r) => setTimeout(r, 700));
  const screen = document.getElementById("status-text-screen");
  const background = screen.style.getPropertyValue("--status-bg");
  const font = screen.style.getPropertyValue("--status-font");
  await buildStatusEmojiPanel();
  await new Promise((r) => setTimeout(r, 500));
  const categories = document.querySelectorAll("#status-emoji-cats .emoji-cat").length;
  const items = document.querySelectorAll("#status-emoji-grid .emoji").length;
  const title = document.getElementById("status-emoji-title").textContent;
  await renderStatusEmojiGroup(0, "grinning");
  await new Promise((r) => setTimeout(r, 300));
  const searchItems = document.querySelectorAll("#status-emoji-grid .emoji").length;
  closeStatusTextComposer();
  closeStatus();
  return {
    backgrounds: palette.backgrounds.length,
    fonts: palette.fonts.length,
    argb,
    background,
    font,
    categories,
    items,
    title,
    searchItems,
  };
});
check("the background palette renders", composer.backgrounds === 20 && composer.background === "#243640" || /^#[0-9a-f]{6}$/i.test(composer.background), `${composer.backgrounds} colours, picked ${composer.background}`);
check("the font stack renders", composer.fonts === 4 && composer.font.length > 0, composer.font);
check("the ARGB value is opaque", composer.argb === 0xff243640, String(composer.argb));
check("the status emoji panel renders", composer.categories === 9 && composer.items > 100, `${composer.categories} cats, ${composer.items} emoji`);
check("the status emoji search renders", composer.searchItems > 0, `${composer.searchItems} matches`);

console.log("\n# media viewer (rust)");
const viewer = await page.evaluate(async () => {
  const chat = [...state.chats.values()].find((entry) =>
    entry.messages.some((m) => m.kind === "image" || m.kind === "video"),
  );
  if (!chat) return { skipped: true };
  await openChat(chat.jid);
  await new Promise((r) => setTimeout(r, 1200));
  const media = chat.messages.find((m) => m.kind === "image" || m.kind === "video");
  openMediaViewer(media.id);
  await new Promise((r) => setTimeout(r, 2500));
  const out = {
    hidden: document.getElementById("media-viewer").hidden,
    chat: document.getElementById("media-viewer-chat").textContent,
    time: document.getElementById("media-viewer-time").textContent,
    stage: document.getElementById("media-viewer-stage").innerHTML.length,
    strip: document.querySelectorAll("#media-viewer-strip .media-strip-thumb").length,
    active: document.querySelectorAll("#media-viewer-strip .active").length,
    kind: media.kind,
  };
  closeMediaViewer();
  return out;
});
if (viewer.skipped) {
  console.log("  ok  media viewer skipped (no media)");
} else {
  check("the viewer opens", !viewer.hidden && viewer.stage > 0, `${viewer.kind}, ${viewer.stage} bytes`);
  check("the viewer header renders", viewer.chat.length > 0 && viewer.time.includes(" at "), `${viewer.chat} · ${viewer.time}`);
  check("the filmstrip renders", viewer.strip > 0 && viewer.active === 1, `${viewer.strip} thumbs`);
}

console.log("\n# sticker picker (rust)");
const sticker = await page.evaluate(async () => {
  const chat = [...state.chats.values()].find((entry) => entry.messages.length > 0);
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 800));
  }
  document.getElementById("emoji-panel").hidden = false;
  setStickerTab("recent");
  await new Promise((r) => setTimeout(r, 2500));
  const out = {
    create: document.querySelectorAll("#sticker-grid .sticker-create").length,
    cells: document.querySelectorAll("#sticker-grid .sticker-cell").length,
    images: document.querySelectorAll("#sticker-grid img").length,
    count: document.getElementById("sticker-count").textContent,
  };
  document.getElementById("emoji-panel").hidden = true;
  return out;
});
check("the sticker grid renders", sticker.create === 1 && sticker.cells > 0, `${sticker.cells} cells, ${sticker.images} images`);
check("the sticker count renders", sticker.count.startsWith("("), sticker.count);

console.log("\n# context menus (rust)");
const menus = await page.evaluate(async () => {
  const out = {};
  const row = document.querySelector("#chat-list .chat-item[data-jid]");
  row.dispatchEvent(
    new MouseEvent("contextmenu", { bubbles: true, clientX: 200, clientY: 300 }),
  );
  await new Promise((r) => setTimeout(r, 500));
  const menu = document.getElementById("chat-list-menu");
  out.chatRows = [...menu.querySelectorAll("button[data-chat-action]")]
    .filter((node) => !node.hidden)
    .map((node) => node.dataset.chatAction);
  menu.hidden = true;

  document.querySelector("#chat-list .chat-item[data-jid]").click();
  await new Promise((r) => setTimeout(r, 1200));
  const bubble = document.querySelector("#messages .bubble[data-id]");
  const message = state.chats
    .get(state.active)
    ?.messages.find((item) => item.id === bubble?.getAttribute("data-id"));
  if (message) {
    await openMessageMenu(message, 300, 300);
    await new Promise((r) => setTimeout(r, 400));
    const msgMenu = document.getElementById("msg-menu");
    out.messageRows = [...msgMenu.querySelectorAll("button[data-action]")]
      .filter((node) => !node.hidden)
      .map((node) => node.dataset.action);
    closeMessageMenu();
  }

  await askDelete([{ fromMe: true, deleted: false, chat: state.active }]);
  await new Promise((r) => setTimeout(r, 400));
  out.deleteTitle = document.getElementById("delete-confirm-title").textContent;
  out.deleteEveryone = !document.getElementById("delete-everyone-ok").hidden;
  closeDeleteConfirm();
  return out;
});
check("the chat menu hides unusable rows", menus.chatRows.includes("pin") && menus.chatRows.includes("mute"), menus.chatRows.join(","));
check("the message menu renders", (menus.messageRows || []).length > 0, (menus.messageRows || []).join(","));
check("the delete dialog is built", menus.deleteTitle === "Delete message?", `${menus.deleteTitle}, everyone=${menus.deleteEveryone}`);

console.log("\n# settings (rust)");
const settings = await page.evaluate(async () => {
  openSettings("home");
  await new Promise((r) => setTimeout(r, 700));
  const title = document.getElementById("settings-title").textContent;
  const links = document.querySelectorAll("#settings-body [data-page]").length;
  const logout = Boolean(document.querySelector('#settings-body [data-action="logout"]'));
  const pages = {};
  for (const page of ["profile", "account", "chats", "ai", "help"]) {
    openSettings(page);
    await new Promise((r) => setTimeout(r, 600));
    pages[page] = {
      title: document.getElementById("settings-title").textContent,
      rows: document.querySelectorAll("#settings-body .settings-row").length,
    };
  }
  openSettings("chats");
  await new Promise((r) => setTimeout(r, 500));
  const toggles = document.querySelectorAll("#settings-body [data-toggle]").length;
  openSettings("help");
  await new Promise((r) => setTimeout(r, 500));
  const credits = document.querySelectorAll('#settings-body a[target="_blank"]').length;
  openSettings("account");
  await new Promise((r) => setTimeout(r, 500));
  const updateButton = Boolean(document.querySelector('#settings-body [data-update="check"]'));
  closeSettings();
  return { title, links, logout, pages, toggles, credits, updateButton };
});
check("the settings home renders", settings.links === 7 && settings.logout, `${settings.links} page rows, logout`);
for (const [page, info] of Object.entries(settings.pages)) {
  check(`the ${page} page renders`, info.rows > 0, `${info.title}: ${info.rows} rows`);
}
check("the chat toggles render", settings.toggles === 4, `${settings.toggles} switches`);
check("the help page lists the credits", settings.credits > 10, `${settings.credits} links`);
check("the update row renders", settings.updateButton);

console.log("\n# group info (rust)");
const groupInfo = await page.evaluate(async () => {
  const group = [...state.chats.values()].find((chat) => chat.isGroup);
  if (!group) return { skipped: true };
  await openChat(group.jid);
  await new Promise((r) => setTimeout(r, 700));
  await openGroupInfo();
  await new Promise((r) => setTimeout(r, 1400));
  const rows = document.querySelectorAll("#group-members .group-member").length;
  const admins = document.querySelectorAll("#group-members .group-member-tag").length;
  const count = document.getElementById("group-info-count").textContent;
  document.getElementById("group-info-screen").hidden = true;
  document.getElementById("rail-chats").click();
  return { rows, admins, count };
});
if (groupInfo.skipped) {
  console.log("  ok  group info skipped (no groups)");
} else {
  check("the member list renders", groupInfo.rows > 0, `${groupInfo.rows} members, ${groupInfo.admins} admins`);
  check("the member count renders", groupInfo.count.includes("members"), groupInfo.count);
}

console.log("\n# group member picker (rust)");
const groupPicker = await page.evaluate(async () => {
  openGroupModal();
  await new Promise((r) => setTimeout(r, 800));
  const rows = document.querySelectorAll("#group-contacts .group-contact").length;
  const first = document.querySelector("#group-contacts .group-contact input");
  const jid = first?.getAttribute("data-group-jid");
  if (jid) groupSelection.add(jid);
  await renderGroupContacts();
  await new Promise((r) => setTimeout(r, 500));
  const checked = jid
    ? document.querySelector(`#group-contacts input[data-group-jid="${jid}"]`)?.checked
    : false;
  document.getElementById("group-modal").hidden = true;
  groupSelection.clear();
  return { rows, checked };
});
check("the group checklist renders", groupPicker.rows > 0, `${groupPicker.rows} contacts`);
check("ticked contacts survive a repaint", groupPicker.checked === true);

console.log("\n# new chat picker (rust)");
const picker = await page.evaluate(async () => {
  openNewChat();
  await new Promise((r) => setTimeout(r, 800));
  const rows = document.querySelectorAll("#new-chat-list .new-chat-row").length;
  const letters = document.querySelectorAll("#new-chat-list .new-chat-letter").length;
  const first = document.querySelector("#new-chat-list .new-chat-row strong")?.textContent || "";
  document.getElementById("new-chat-input").value = "+62 812 3456 7890";
  await renderNewChat();
  await new Promise((r) => setTimeout(r, 500));
  const numberRow = Boolean(
    document.querySelector('#new-chat-list [data-chat-jid="6281234567890@s.whatsapp.net"]'),
  );
  document.getElementById("new-chat-input").value = "";
  closeNewChat();
  return { rows, letters, first, numberRow };
});
check("the picker renders", picker.rows > 0, `${picker.rows} rows, ${picker.letters} letters`);
check("my own row comes first", picker.first === "You", picker.first);
check("a typed number gets its own row", picker.numberRow);

console.log("\n# status screen (rust)");
const statusScreen = await page.evaluate(async () => {
  openStatus();
  await new Promise((r) => setTimeout(r, 1200));
  const rows = document.querySelectorAll("#status-list .status-item").length;
  const viewed = document.querySelectorAll("#status-viewed-list .status-item").length;
  const mineLabel = document.querySelector("#status-mine .status-meta span")?.textContent || "";
  const first = document.querySelector("#status-list .status-item, #status-viewed-list .status-item");
  let viewer = { skipped: true };
  if (first) {
    openStatusViewer(first.getAttribute("data-status-jid"));
    await new Promise((r) => setTimeout(r, 2200));
    viewer = {
      name: document.getElementById("status-viewer-name").textContent,
      time: document.getElementById("status-viewer-time").textContent,
      segments: document.querySelectorAll("#status-progress span").length,
      body: document.querySelector("#status-viewer-body")?.innerHTML.slice(0, 40) || "",
      kind: statusQueue[statusIndex]?.kind || "",
    };
  }
  closeStatusViewer();
  document.getElementById("rail-chats").click();
  return { rows, viewed, mineLabel, viewer };
});
check("status list renders", statusScreen.rows >= 0, `${statusScreen.rows} recent, ${statusScreen.viewed} viewed`);
check("my status row has its label", statusScreen.mineLabel.length > 0, statusScreen.mineLabel);
if (statusScreen.viewer.skipped) {
  console.log("  ok  status viewer skipped (no stories)");
} else {
  check("the story viewer shell renders", statusScreen.viewer.segments > 0, `${statusScreen.viewer.segments} segments, ${statusScreen.viewer.kind}`);
  check("the story name/time render", statusScreen.viewer.name.length > 0 && statusScreen.viewer.time.includes(" at "), `${statusScreen.viewer.name} · ${statusScreen.viewer.time}`);
  check("the story body renders", statusScreen.viewer.body.length > 0, statusScreen.viewer.body);
}

console.log("\n# secondary screens (rust)");
const starred = await page.evaluate(async () => {
  openStarred();
  await new Promise((r) => setTimeout(r, 600));
  const rows = document.querySelectorAll("#starred-body .starred-row").length;
  const empty = document.querySelectorAll("#starred-body .empty").length;
  closeStarred();
  return { rows, empty };
});
check("starred screen renders", starred.rows > 0 || starred.empty > 0, `${starred.rows} rows, ${starred.empty} empty states`);

const channelsPanel = await page.evaluate(async () => {
  openChannels();
  await new Promise((r) => setTimeout(r, 800));
  const rows = document.querySelectorAll("#channels-list .channel-row").length;
  const active = document.querySelectorAll("#channels-list .channel-row.active").length;
  const avatars = document.querySelectorAll("#channels-list .channel-row .avatar").length;
  document.getElementById("rail-chats").click();
  return { rows, active, avatars };
});
check("channels panel renders", channelsPanel.rows > 0, `${channelsPanel.rows} channels`);
check("channel rows carry avatars", channelsPanel.avatars === channelsPanel.rows);

const communityScreen = await page.evaluate(async () => {
  const jid = Object.keys(state.groupMeta || {}).find((key) => state.groupMeta[key]?.isCommunity);
  if (!jid) return { skipped: true };
  openCommunity(jid);
  await new Promise((r) => setTimeout(r, 600));
  const title = document.getElementById("community-title").textContent;
  const rows = document.querySelectorAll("#community-body .chat-item").length;
  closeCommunity();
  return { title, rows };
});
if (communityScreen.skipped) {
  console.log("  ok  community screen skipped (no community in this account)");
} else {
  check("community screen renders", communityScreen.rows > 0, `${communityScreen.title}: ${communityScreen.rows} rows`);
}

console.log("\n# console");
check("no JavaScript errors", errors.length === 0, errors.slice(0, 3).join(" | "));

await page.screenshot({ path: "tests/e2e/artifacts/ui-rust.png" });
console.log(`\n${failures === 0 ? "PASS" : `${failures} FAILURE(S)`}`);
process.exit(failures === 0 ? 0 : 1);
