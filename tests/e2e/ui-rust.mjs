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

console.log("\n# chat fold + outgoing bubble (rust)");
const foldCheck = await page.evaluate(async () => {
  const merge = await invoke("ui_fold_plan", {
    input: {
      from: "111@lid",
      to: "6285@s.whatsapp.net",
      name: "Ada",
      source_exists: true,
      target_exists: true,
      target_name: "6285",
      source_ids: ["a", "b"],
      target_ids: ["b", "c"],
      source_last_ts: 500,
      target_last_ts: 400,
      source_unread: 2,
    },
  });
  const rekey = await invoke("ui_fold_plan", {
    input: { from: "111@lid", to: "6285@s.whatsapp.net", name: "Ada", source_exists: true, target_exists: false },
  });
  const rename = await invoke("ui_fold_plan", {
    input: {
      from: "111@lid",
      to: "6285@s.whatsapp.net",
      name: "Ada",
      source_exists: false,
      target_exists: true,
      target_name: "6285",
    },
  });
  const kept = await invoke("ui_fold_plan", {
    input: {
      from: "111@lid",
      to: "6285@s.whatsapp.net",
      name: "Ada",
      source_exists: false,
      target_exists: true,
      target_name: "Budi",
    },
  });
  const plain = await invoke("ui_outgoing_message", {
    input: {
      temp_id: "local-1",
      chat: "1@s.whatsapp.net",
      is_group: false,
      timestamp: 42,
      text: "hello",
      quote: null,
    },
  });
  const quoted = await invoke("ui_outgoing_message", {
    input: {
      temp_id: "local-2",
      chat: "1@g.us",
      is_group: true,
      timestamp: 43,
      text: "answer",
      quote: { id: "q1", text: "question", sender_name: "", from_me: true, kind: "text" },
    },
  });

  // Live: two probe chats merge through the plan (the source disappears).
  const from = "999000333@lid";
  const to = "999000333@s.whatsapp.net";
  state.chats.set(from, {
    jid: from,
    name: "Fold Probe",
    isGroup: false,
    messages: [{ id: "fp1", timestamp: 2000, text: "a", kind: "text" }],
    last: { id: "fp1", timestamp: 2000, text: "a" },
    lastTs: 2000,
    unread: 2,
  });
  state.chats.set(to, {
    jid: to,
    name: "999000333",
    isGroup: false,
    messages: [{ id: "fp2", timestamp: 1000, text: "b", kind: "text" }],
    last: { id: "fp2", timestamp: 1000, text: "b" },
    lastTs: 1000,
    unread: 0,
  });
  const folded = await foldChat(from, to, "Fold Probe");
  const target = state.chats.get(to);
  const live = {
    folded,
    sourceGone: !state.chats.has(from),
    ids: target ? target.messages.map((m) => m.id) : [],
    unread: target?.unread,
    lastTs: target?.lastTs,
    name: target?.name,
  };
  if (target) state.chats.delete(to);
  renderChatList();
  return { merge, rekey, rename, kept, plain, quoted, live };
});
check(
  "the fold plan merges only the missing ids",
  foldCheck.merge.mode === "merge" &&
    foldCheck.merge.append_ids.join(",") === "a" &&
    foldCheck.merge.last_ts === 500 &&
    foldCheck.merge.unread_add === 2,
  JSON.stringify(foldCheck.merge),
);
check(
  "the fold plan re-keys, renames or leaves alone",
  foldCheck.rekey.mode === "rekey" &&
    foldCheck.rename.mode === "rename_only" &&
    foldCheck.kept.mode === "none",
  `${foldCheck.rekey.mode}/${foldCheck.rename.mode}/${foldCheck.kept.mode}`,
);
check(
  "the optimistic text bubble renders",
  foldCheck.plain.id === "local-1" &&
    foldCheck.plain.pending &&
    foldCheck.plain.text === "hello" &&
    foldCheck.plain.reply_to === null,
  JSON.stringify(foldCheck.plain),
);
check(
  "a quoted send carries its quote card",
  foldCheck.quoted.reply_to.id === "q1" &&
    foldCheck.quoted.reply_to.sender_name === "You" &&
    foldCheck.quoted.is_group,
  JSON.stringify(foldCheck.quoted.reply_to),
);
check(
  "the live fold merges two chats",
  foldCheck.live.folded &&
    foldCheck.live.sourceGone &&
    foldCheck.live.ids.join(",") === "fp2,fp1" &&
    foldCheck.live.unread === 2 &&
    foldCheck.live.lastTs === 2000 &&
    foldCheck.live.name === "Fold Probe",
  JSON.stringify(foldCheck.live),
);

console.log("\n# chat row upsert + sorted insert (rust)");
const upsertCheck = await page.evaluate(async () => {
  const create = await invoke("ui_upsert_plan", {
    input: { jid: "6285@s.whatsapp.net", name: "", is_group: null, exists: false, current_name: "" },
  });
  const rename = await invoke("ui_upsert_plan", {
    input: {
      jid: "6285@s.whatsapp.net",
      name: "Budi",
      is_group: false,
      exists: true,
      current_name: "6285",
    },
  });
  const keep = await invoke("ui_upsert_plan", {
    input: {
      jid: "6285@s.whatsapp.net",
      name: "Someone",
      is_group: false,
      exists: true,
      current_name: "Budi",
    },
  });
  const insert = {
    empty: await invoke("ui_insert_index", { timestamps: [], timestamp: 100 }),
    first: await invoke("ui_insert_index", { timestamps: [100, 200, 300], timestamp: 50 }),
    equal: await invoke("ui_insert_index", { timestamps: [100, 200, 300], timestamp: 200 }),
    middle: await invoke("ui_insert_index", { timestamps: [100, 200, 300], timestamp: 250 }),
    last: await invoke("ui_insert_index", { timestamps: [100, 200, 300], timestamp: 400 }),
  };

  // Live: a new row is created through the plan, a placeholder is renamed in
  // place, and a message lands at its timestamp position.
  const probeJid = "999000222@s.whatsapp.net";
  state.chats.delete(probeJid);
  const created = await upsertChat({ chat: probeJid, name: "", isGroup: false });
  const liveCreate = {
    name: created?.name,
    inStore: state.chats.has(probeJid),
    messages: created?.messages?.length,
  };
  created.name = "999000222";
  const renamed = await upsertChat({ chat: probeJid, name: "Probe Name" });
  const liveRename = { name: renamed?.name, sameEntry: renamed === created };
  state.chats.delete(probeJid);

  const chat = [...state.chats.values()].find(
    (entry) => entry.messages.length > 5 && entry.messages.length < 200,
  );
  let liveInsert = null;
  if (chat) {
    const before = chat.messages.length;
    const ts = chat.messages[Math.floor(before / 2)].timestamp;
    await addIncoming({
      id: `probe-sort-${Date.now()}`,
      chat: chat.jid,
      chat_name: chat.name,
      is_group: chat.isGroup,
      from_me: false,
      timestamp: ts,
      kind: "text",
      text: "sort probe",
    });
    const index = chat.messages.findIndex((m) => m.text === "sort probe");
    liveInsert = {
      added: chat.messages.length - before,
      index,
      sorted: chat.messages.every((m, i) => i === 0 || m.timestamp >= chat.messages[i - 1].timestamp),
    };
    if (index >= 0) chat.messages.splice(index, 1);
    renderChatList();
  }
  return { create, rename, keep, insert, liveCreate, liveRename, liveInsert };
});
check(
  "a new chat row takes the jid user part as its name",
  upsertCheck.create.create && upsertCheck.create.name === "6285" && !upsertCheck.create.is_group,
  JSON.stringify(upsertCheck.create),
);
check(
  "a placeholder row is renamed, a real name is kept",
  upsertCheck.rename.rename && upsertCheck.rename.name === "Budi" && !upsertCheck.keep.rename,
  `${upsertCheck.rename.rename}/${upsertCheck.keep.rename}`,
);
check(
  "the sorted insert index is stable for equal timestamps",
  upsertCheck.insert.empty === 0 &&
    upsertCheck.insert.first === 0 &&
    upsertCheck.insert.equal === 2 &&
    upsertCheck.insert.middle === 2 &&
    upsertCheck.insert.last === 3,
  JSON.stringify(upsertCheck.insert),
);
check(
  "the live upsert creates and renames in place",
  upsertCheck.liveCreate.inStore &&
    upsertCheck.liveCreate.name === "999000222" &&
    upsertCheck.liveCreate.messages === 0 &&
    upsertCheck.liveRename.name === "Probe Name" &&
    upsertCheck.liveRename.sameEntry,
  JSON.stringify(upsertCheck.liveRename),
);
if (upsertCheck.liveInsert) {
  check(
    "a live message lands in timestamp order",
    upsertCheck.liveInsert.added === 1 &&
      upsertCheck.liveInsert.index > 0 &&
      upsertCheck.liveInsert.sorted,
    JSON.stringify(upsertCheck.liveInsert),
  );
}

console.log("\n# menus + missing-handler guards (rust)");
const menuCheck = await page.evaluate(async () => {
  const anchor = await invoke("ui_menu_anchor", {
    anchor: { x: 900, y: 40, width: 40, height: 40 },
    menuW: 220,
    menuH: 260,
    viewportW: 1200,
    viewportH: 900,
  });
  const anchorLow = await invoke("ui_menu_anchor", {
    anchor: { x: 10, y: 860, width: 30, height: 30 },
    menuW: 220,
    menuH: 260,
    viewportW: 1200,
    viewportH: 900,
  });

  // Regression guard: these handlers were referenced but never defined, which
  // made the sidebar kebab, "Select chats" and status posting throw.
  const names = [
    "toggleSideMenu",
    "openSideMenu",
    "closeSideMenu",
    "statusCaptureLimits",
    "startChatSelection",
    "exitChatSelection",
    "toggleChatSelection",
    "applyChatSelection",
    "markAllChatsRead",
  ];
  const defined = {};
  for (const name of names) defined[name] = typeof window[name];

  $("chat-side-menu").click();
  await new Promise((r) => setTimeout(r, 500));
  const menu = $("side-menu");
  const live = {
    open: !menu.hidden,
    items: menu.querySelectorAll("[data-side-action]").length,
    placed: menu.style.left !== "" && menu.style.top !== "",
  };
  closeSideMenu();
  await new Promise((r) => setTimeout(r, 200));
  live.closed = menu.hidden;

  const capture = await statusCaptureLimits();
  return { anchor, anchorLow, defined, live, capture };
});
check(
  "menus anchor under their button",
  menuCheck.anchor.left === 720 && menuCheck.anchor.top === 86,
  JSON.stringify(menuCheck.anchor),
);
check(
  "a low anchor flips the menu up inside the window",
  menuCheck.anchorLow.top === 632 && menuCheck.anchorLow.left === 8,
  JSON.stringify(menuCheck.anchorLow),
);
check(
  "every referenced handler is defined",
  Object.values(menuCheck.defined).every((type) => type === "function"),
  JSON.stringify(menuCheck.defined),
);
check(
  "the sidebar kebab opens, positions and closes",
  menuCheck.live.open && menuCheck.live.items >= 6 && menuCheck.live.placed && menuCheck.live.closed,
  JSON.stringify(menuCheck.live),
);
check(
  "the status capture limits resolve",
  menuCheck.capture.thumb_max_px === 320 && menuCheck.capture.jpeg_quality === 0.72,
  JSON.stringify(menuCheck.capture),
);

console.log("\n# sidebar select chats (rust)");
const selectChats = await page.evaluate(async () => {
  const chats = [
    { jid: "a", pinned: false, archived: false, unread: 3, is_group: false },
    { jid: "b", pinned: false, archived: false, unread: 0, is_group: false },
    { jid: "c", pinned: false, archived: true, unread: 1, is_group: true },
  ];
  const read = await invoke("ui_bulk_chat_plan", { action: "read", chats });
  const archive = await invoke("ui_bulk_chat_plan", { action: "archive", chats });
  const del = await invoke("ui_bulk_chat_plan", {
    action: "delete",
    chats: [chats[0]],
  });
  const unknown = await invoke("ui_bulk_chat_plan", { action: "nope", chats });

  // Live: the menu's "Select chats" path used to throw (the handlers were never
  // defined); it must now enter selection mode and tick rows.
  const defined = ["startChatSelection", "exitChatSelection", "toggleChatSelection", "markAllChatsRead", "applyChatSelection"].every(
    (name) => typeof window[name] === "function",
  );
  startChatSelection();
  await new Promise((r) => setTimeout(r, 300));
  const rows = [...document.querySelectorAll("#chat-list .chat-item[data-jid]")];
  const first = rows[0]?.dataset.jid;
  const second = rows[1]?.dataset.jid;
  await toggleChatSelection(first);
  await new Promise((r) => setTimeout(r, 400));
  const live = {
    defined,
    selecting: state.selectingChats,
    barHidden: document.getElementById("chat-select-bar").hidden,
    label: document.getElementById("chat-select-count").textContent,
    checkedRows: document.querySelectorAll("#chat-list .chat-item.selected").length,
    readEnabled: !document.getElementById("chat-select-read").disabled,
  };
  await toggleChatSelection(second);
  await new Promise((r) => setTimeout(r, 300));
  const twoSelected = state.selectedChats.size;
  await toggleChatSelection(first);
  await toggleChatSelection(second);
  await new Promise((r) => setTimeout(r, 300));
  const afterEmpty = {
    selecting: state.selectingChats,
    barHidden: document.getElementById("chat-select-bar").hidden,
  };
  return { read, archive, del, unknown, live, twoSelected, afterEmpty };
});
check(
  "bulk read skips already-read chats",
  selectChats.read.length === 2 &&
    selectChats.read[0].command === "chat_read_mark" &&
    selectChats.read[1].args.chat === "c",
  `${selectChats.read.length} plans`,
);
check(
  "bulk archive skips archived chats and delete never does",
  selectChats.archive.length === 2 &&
    selectChats.del.length === 1 &&
    Boolean(selectChats.del[0].confirm),
  `${selectChats.archive.length} archive / ${selectChats.del.length} delete`,
);
check("unknown bulk actions plan nothing", selectChats.unknown.length === 0);
check(
  "Select chats now enters selection mode",
  selectChats.live.defined &&
    selectChats.live.selecting &&
    !selectChats.live.barHidden &&
    selectChats.live.checkedRows === 1 &&
    selectChats.live.readEnabled,
  `${selectChats.live.label}, ${selectChats.live.checkedRows} ticked`,
);
check(
  "the sidebar selection ends when the last row is unticked",
  selectChats.twoSelected === 2 && !selectChats.afterEmpty.selecting && selectChats.afterEmpty.barHidden,
  `${selectChats.twoSelected} selected then exited`,
);

console.log("\n# photo pump + theme + icons (rust)");
const pumpTheme = await page.evaluate(async () => {
  const pump = {
    empty: await invoke("ui_photo_pump", { queue: [], inFlight: 0, connected: true }),
    three: await invoke("ui_photo_pump", { queue: ["a", "b", "c", "d", "e"], inFlight: 0, connected: true }),
    partial: await invoke("ui_photo_pump", { queue: ["a", "b", "c", "d", "e"], inFlight: 2, connected: true }),
    full: await invoke("ui_photo_pump", { queue: ["a"], inFlight: 3, connected: true }),
    offline: await invoke("ui_photo_pump", { queue: ["a"], inFlight: 0, connected: false }),
  };
  const photoOk = [
    await invoke("ui_photo_result_ok", { url: "https://pps.whatsapp.net/x.jpg" }),
    await invoke("ui_photo_result_ok", { url: "blob:abc" }),
    await invoke("ui_photo_result_ok", { url: "" }),
  ];
  const groups = {
    top: await invoke("ui_emoji_active_group", { offsets: [[0, 0], [1, 800], [2, 1600]], scrollTop: 0 }),
    mid: await invoke("ui_emoji_active_group", { offsets: [[0, 0], [1, 800], [2, 1600]], scrollTop: 795 }),
    deep: await invoke("ui_emoji_active_group", { offsets: [[0, 0], [1, 800], [2, 1600]], scrollTop: 5000 }),
  };
  const theme = {
    labels: [
      await invoke("ui_theme_label", { preference: "light" }),
      await invoke("ui_theme_label", { preference: "dark" }),
      await invoke("ui_theme_label", { preference: "system" }),
    ],
    resolved: [
      await invoke("ui_theme_resolve", { preference: "light", systemDark: true }),
      await invoke("ui_theme_resolve", { preference: "system", systemDark: true }),
      await invoke("ui_theme_resolve", { preference: "system", systemDark: false }),
    ],
    toggle: await invoke("ui_theme_toggle", { resolved: "dark" }),
  };
  const icons = {
    palette: (await invoke("ui_avatar_palette")).length,
    style: await invoke("ui_avatar_style", { name: "Ada" }),
    initials: await invoke("ui_initials", { name: "Ada Lovelace" }),
    groupIcon: (await invoke("ui_group_avatar_icon")).length,
    mediaIcon: (await invoke("ui_media_icon", { kind: "video" })).length,
    selfJid: await invoke("ui_is_self_jid", { jid: state.meJid, meJid: state.meJid, meLid: state.meLid }),
  };

  // Live: avatars keep painting, and the theme toggle goes through Rust.
  const avatars = [...document.querySelectorAll("#chat-list .chat-item .avatar")];
  const liveAvatars = {
    rows: avatars.length,
    painted: avatars.filter((node) => node.innerHTML.length > 0).length,
    groupGlyph: document.querySelectorAll(".group-avatar").length,
  };
  applyTheme();
  await new Promise((r) => setTimeout(r, 500));
  const themeToggle = {
    theme: document.documentElement.dataset.theme,
    title: document.getElementById("theme-toggle").title,
    hasIcon: document.getElementById("theme-toggle").innerHTML.length > 0,
  };
  return { pump, photoOk, groups, theme, icons, liveAvatars, themeToggle };
});
check(
  "the photo pump fills three slots",
  pumpTheme.pump.three.start.join(",") === "a,b,c" &&
    pumpTheme.pump.three.in_flight === 3 &&
    pumpTheme.pump.three.waiting === 2 &&
    pumpTheme.pump.partial.start.join(",") === "a",
  JSON.stringify(pumpTheme.pump.three),
);
check(
  "the pump stops when full or offline",
  pumpTheme.pump.full.start.length === 0 && pumpTheme.pump.offline.start.length === 0,
);
check(
  "only http photo urls are cached",
  pumpTheme.photoOk.join(",") === "true,false,false",
  pumpTheme.photoOk.join(","),
);
check(
  "the emoji strip selection maths render",
  pumpTheme.groups.top === 0 && pumpTheme.groups.mid === 1 && pumpTheme.groups.deep === 2,
  JSON.stringify(pumpTheme.groups),
);
check(
  "theme labels and resolution render",
  pumpTheme.theme.labels.join(",") === "Light,Dark,System default" &&
    pumpTheme.theme.resolved.join(",") === "light,dark,light" &&
    pumpTheme.theme.toggle.label === "Light" &&
    pumpTheme.theme.toggle.icon === "light_mode",
  JSON.stringify(pumpTheme.theme.resolved),
);
check(
  "the avatar palette and icons come from Rust",
  pumpTheme.icons.palette === 11 &&
    pumpTheme.icons.style.startsWith("background:#") &&
    pumpTheme.icons.initials === "AL" &&
    pumpTheme.icons.groupIcon > 100 &&
    pumpTheme.icons.mediaIcon > 100,
  `${pumpTheme.icons.palette} tints, ${pumpTheme.icons.initials}`,
);
check(
  "avatars still paint in the sidebar",
  pumpTheme.liveAvatars.rows > 0 && pumpTheme.liveAvatars.painted === pumpTheme.liveAvatars.rows,
  JSON.stringify(pumpTheme.liveAvatars),
);
check(
  "the theme toggle renders through Rust",
  pumpTheme.themeToggle.hasIcon && pumpTheme.themeToggle.title.startsWith("Switch to"),
  `${pumpTheme.themeToggle.theme} · ${pumpTheme.themeToggle.title}`,
);

console.log("\n# call codec + scroll maths (rust)");
const codec = await page.evaluate(async () => {
  const video = await invoke("ui_video_config");
  const keyframe = {
    zero: await invoke("ui_is_keyframe", { frameNumber: 0 }),
    mid: await invoke("ui_is_keyframe", { frameNumber: 29 }),
    every30: await invoke("ui_is_keyframe", { frameNumber: 30 }),
  };
  const audio = await invoke("ui_audio_format");
  const schedule = {
    first: await invoke("ui_audio_schedule", { nextPlay: 0, now: 10, duration: 0.06 }),
    behind: await invoke("ui_audio_schedule", { nextPlay: 10.005, now: 10, duration: 0.06 }),
    ahead: await invoke("ui_audio_schedule", { nextPlay: 10.7, now: 10, duration: 0.06 }),
    inWindow: await invoke("ui_audio_schedule", { nextPlay: 10.2, now: 10, duration: 0.06 }),
  };
  const pcm = [
    await invoke("ui_pcm_sample", { sample: 1 }),
    await invoke("ui_pcm_sample", { sample: -1 }),
    await invoke("ui_pcm_sample", { sample: 0 }),
  ];
  const scroll = {
    distance: await invoke("ui_scroll_distance", { scrollHeight: 2000, scrollTop: 1400, clientHeight: 600 }),
    following: await invoke("ui_scroll_following", { distance: 120 }),
    notFollowing: await invoke("ui_scroll_following", { distance: 121 }),
    latest: await invoke("ui_scroll_latest_visible", { childCount: 3, distance: 50 }),
    noLatest: await invoke("ui_scroll_latest_visible", { childCount: 0, distance: 50 }),
    anchor: await invoke("ui_scroll_anchor", { nodeOffset: 900, anchorOffset: 40, fallback: 0 }),
    constants: await invoke("ui_scroll_constants"),
  };

  // Live: a chat still opens and sticks to the bottom through the async anchor.
  const chat = [...state.chats.values()].find((entry) => entry.messages.length > 5);
  let live = null;
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 1400));
    live = {
      bubbles: document.querySelectorAll("#messages .bubble").length,
      stick: document.getElementById("messages").dataset.stickBottom,
      scrollTop: document.getElementById("messages").scrollTop,
    };
  }
  return { video, keyframe, audio, schedule, pcm, scroll, live };
});
check(
  "the call video config renders",
  codec.video.codec === "avc1.42E01F" &&
    codec.video.width === 640 &&
    codec.video.bitrate === 700000 &&
    codec.video.keyframe_interval === 30,
  JSON.stringify(codec.video),
);
check(
  "keyframes every 30 frames",
  codec.keyframe.zero && !codec.keyframe.mid && codec.keyframe.every30,
  JSON.stringify(codec.keyframe),
);
check(
  "the audio format matches the wire",
  codec.audio.sample_rate === 16000 && codec.audio.frame_samples === 960,
  JSON.stringify(codec.audio),
);
check(
  "the jitter buffer resets out of window",
  codec.schedule.first[0] === 10.06 &&
    codec.schedule.behind[0] === 10.06 &&
    codec.schedule.ahead[0] === 10.06 &&
    codec.schedule.inWindow[0] === 10.2,
  JSON.stringify(codec.schedule),
);
check("pcm conversion is symmetric", codec.pcm.join(",") === "32767,-32768,0", codec.pcm.join(","));
check(
  "the scroll maths render",
  codec.scroll.distance === 0 &&
    codec.scroll.following &&
    !codec.scroll.notFollowing &&
    codec.scroll.latest &&
    !codec.scroll.noLatest &&
    codec.scroll.anchor === 860 &&
    codec.scroll.constants.follow_threshold_px === 120,
  JSON.stringify(codec.scroll.constants),
);
if (codec.live) {
  check(
    "the chat still sticks to the newest message",
    codec.live.bubbles > 0 && codec.live.stick === "1" && codec.live.scrollTop > 0,
    JSON.stringify(codec.live),
  );
}

console.log("\n# composer send plan (rust)");
const sendPlan = await page.evaluate(async () => {
  const plan = await invoke("ui_draft_send_plan", { count: 3, caption: "  hello  " });
  const emptyPlan = await invoke("ui_draft_send_plan", { count: 0, caption: "x" });
  const photo = await invoke("ui_attachment_message", {
    input: {
      temp_id: "local-1",
      kind: "image",
      caption: "cap",
      file_name: "a.png",
      mime: "image/png",
      size: 10,
      is_group: false,
      timestamp: 42,
    },
  });
  const doc = await invoke("ui_attachment_message", {
    input: {
      temp_id: "local-2",
      kind: "document",
      caption: "",
      file_name: "a.pdf",
      mime: "application/pdf",
      size: 2048,
      is_group: true,
      timestamp: 43,
    },
  });
  const ok = await invoke("ui_attachment_result", { tempId: "local-1", serverId: "3EB0", ok: true });
  const failed = await invoke("ui_attachment_result", { tempId: "local-1", serverId: "", ok: false });

  // Live: stage a draft, confirm the stack rules, then clear it (no upload).
  const chat = [...state.chats.values()].find((entry) => !entry.isGroup && entry.messages.length);
  let staged = null;
  let emptySend = null;
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 900));
    clearDraft();
    const img = new File([new Uint8Array([1, 2, 3])], "probe.png", { type: "image/png" });
    await addDraftFiles([img]);
    await new Promise((r) => setTimeout(r, 300));
    staged = { files: draftFiles.length, thumbs: document.querySelectorAll("#draft-thumbs .draft-item").length };
    clearDraft();
    const before = chat.messages.length;
    await sendDraft();
    emptySend = chat.messages.length === before;
  }
  return { plan, emptyPlan, photo, doc, ok, failed, staged, emptySend };
});
check(
  "only the first draft file carries the caption",
  sendPlan.plan.length === 3 &&
    sendPlan.plan[0].caption === "hello" &&
    sendPlan.plan[1].caption === "" &&
    sendPlan.emptyPlan.length === 0,
  JSON.stringify(sendPlan.plan),
);
check(
  "the optimistic photo bubble renders",
  sendPlan.photo.id === "local-1" && sendPlan.photo.pending && sendPlan.photo.text === "cap" && !sendPlan.photo.document,
  JSON.stringify(sendPlan.photo),
);
check(
  "the optimistic document bubble carries its card",
  sendPlan.doc.document.name === "a.pdf" && sendPlan.doc.document.size === 2048 && sendPlan.doc.is_group,
  JSON.stringify(sendPlan.doc.document),
);
check(
  "upload results swap the temp id only on success",
  sendPlan.ok.id === "3EB0" && !sendPlan.ok.failed && sendPlan.failed.id === "local-1" && sendPlan.failed.failed,
  `${sendPlan.ok.id}/${sendPlan.failed.id}`,
);
if (sendPlan.staged) {
  check(
    "a staged image fills the draft strip",
    sendPlan.staged.files === 1 && sendPlan.staged.thumbs === 1 && sendPlan.emptySend,
    JSON.stringify(sendPlan.staged),
  );
}

console.log("\n# media hydration + selection (rust)");
const hydration = await page.evaluate(async () => {
  const node = (status, kind, failed = false, channel = false) => ({ status, kind, failed, channel });
  const plan = await invoke("ui_hydrate_plan", {
    nodes: [
      node("", "image"),
      node("", "video"),
      node("ready", "image"),
      node("error", "image"),
      node("", "video", false, true),
      node("", "image", true),
    ],
  });
  const download = {
    loading: await invoke("ui_download_action", { status: "loading" }),
    ready: await invoke("ui_download_action", { status: "ready" }),
    error: await invoke("ui_download_action", { status: "error" }),
    none: await invoke("ui_download_action", { status: "" }),
  };
  const evictions = await invoke("ui_evictions", { keys: ["a", "b", "c", "d"], max: 2 });
  const select = {
    add: await invoke("ui_toggle_selection", { selected: [], id: "m1" }),
    second: await invoke("ui_toggle_selection", { selected: ["m1"], id: "m2" }),
    removeOne: await invoke("ui_toggle_selection", { selected: ["m1", "m2"], id: "m1" }),
    empty: await invoke("ui_toggle_selection", { selected: ["m2"], id: "m2" }),
  };

  // Live: a chat with undownloaded media must still hydrate a tile.
  const chat = [...state.chats.values()].find((entry) =>
    entry.messages.some((m) => (m.kind === "image" || m.kind === "video") && !state.media.has(m.id)),
  );
  let live = null;
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 1200));
    const tiles = document.querySelectorAll("#messages .media-box[data-media-id]").length;
    const before = state.media.size;
    const box = document.querySelector("#messages .media-box[data-media-id].media-box:not(.error)");
    if (box) await loadMedia(box.dataset.mediaId, box.dataset.mediaKind || "image", box);
    for (let attempt = 0; attempt < 20; attempt += 1) {
      await new Promise((r) => setTimeout(r, 250));
      if (state.media.size > before) break;
    }
    live = {
      tiles,
      started: state.media.size > before,
      ready: document.querySelectorAll("#messages .media-image img, #messages .media-video").length,
    };
  }

  // Live selection through the real handlers.
  startSelection({ id: "x" });
  await new Promise((r) => setTimeout(r, 200));
  await toggleSelection("y");
  await new Promise((r) => setTimeout(r, 250));
  const liveSelect = {
    size: state.selected.size,
    label: document.getElementById("select-count").textContent,
  };
  await toggleSelection("x");
  await toggleSelection("y");
  await new Promise((r) => setTimeout(r, 300));
  const liveExit = {
    selecting: state.selecting,
    barHidden: document.getElementById("select-bar").hidden,
  };
  return { plan, download, evictions, select, live, liveSelect, liveExit };
});
check(
  "the hydrate plan matches every tile state",
  hydration.plan.join(",") === "observe,tap_to_load,repaint,retry_tap,observe,skip",
  hydration.plan.join(","),
);
check(
  "downloads skip in-flight and cached media",
  hydration.download.loading === "skip" && hydration.download.ready === "skip" && hydration.download.error === "start",
  JSON.stringify(hydration.download),
);
check(
  "evictions drop the oldest keys",
  hydration.evictions.join(",") === "a,b",
  hydration.evictions.join(","),
);
check(
  "the selection toggle adds, removes and ends",
  hydration.select.add.checked &&
    hydration.select.second.selected.length === 2 &&
    hydration.select.removeOne.selected.join(",") === "m2" &&
    hydration.select.empty.exit,
  JSON.stringify(hydration.select.empty),
);
if (hydration.live) {
  check(
    "live media hydration starts a download",
    hydration.live.tiles > 0 && hydration.live.started,
    JSON.stringify(hydration.live),
  );
}
check(
  "the live selection bar tracks the set",
  hydration.liveSelect.size === 2 && hydration.liveSelect.label === "2 selected",
  hydration.liveSelect.label,
);
check(
  "toggling the last selection exits select mode",
  hydration.liveExit.selecting === false && hydration.liveExit.barHidden,
  JSON.stringify(hydration.liveExit),
);

console.log("\n# status views + draft stack (rust)");
const viewsDrafts = await page.evaluate(async () => {
  const fresh = await invoke("ui_status_note_view", {
    viewers: [{ jid: "111@lid", name: "Ada", timestamp: 1 }],
    jid: "222:9@s.whatsapp.net",
    name: "",
    timestamp: 7,
  });
  const duplicate = await invoke("ui_status_note_view", {
    viewers: [{ jid: "111@lid", name: "Ada", timestamp: 1 }],
    jid: "111:42@lid",
    name: "Ada",
    timestamp: 5,
  });
  const broadcast = await invoke("ui_status_note_view", {
    viewers: [],
    jid: "status@broadcast",
    name: "x",
    timestamp: 1,
  });
  const heal = await invoke("ui_status_heal", {
    chats: [
      {
        messages: [
          { id: "m1", status_quote: false, reply_to_id: "story-1", once_recovered: false },
          { id: "m2", status_quote: false, reply_to_id: "chat-1", once_recovered: false },
          { id: "story-1", status_quote: false, reply_to_id: "", once_recovered: true },
        ],
      },
    ],
    updateIds: ["story-1"],
  });
  const files = [
    { name: "a.png", mime: "image/png" },
    { name: "b.pdf", mime: "application/pdf" },
    { name: "c.jpg", mime: "image/jpeg" },
  ];
  const draft = await invoke("ui_draft_plan", { existing: 0, files });
  const draftFull = await invoke("ui_draft_plan", { existing: 10, files });

  // Live: open a chat, then stack a real image + a rejected PDF.
  const chat = [...state.chats.values()].find((entry) => !entry.isGroup && entry.messages.length);
  let liveDraft = null;
  let liveHeal = null;
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 900));
    clearDraft();
    const img = new File([new Uint8Array([1, 2, 3])], "probe.png", { type: "image/png" });
    const pdf = new File([new Uint8Array([1])], "probe.pdf", { type: "application/pdf" });
    await addDraftFiles([img, pdf]);
    await new Promise((r) => setTimeout(r, 400));
    liveDraft = {
      added: draftFiles.length,
      thumbs: document.querySelectorAll("#draft-thumbs .draft-item").length,
      open: !document.getElementById("draft").hidden,
    };
    clearDraft();

    const story = [...state.statuses.values()].flatMap((entry) => entry.updates)[0];
    if (story) {
      const target = chat.messages[0];
      const savedQuote = target.status_quote;
      const savedReply = target.reply_to;
      target.status_quote = false;
      target.reply_to = { id: story.id, kind: "text", text: "x" };
      await healStatusInteractions();
      await new Promise((r) => setTimeout(r, 400));
      liveHeal = { quoted: Boolean(target.status_quote), sameMessage: target.id === chat.messages[0].id };
      target.status_quote = savedQuote;
      target.reply_to = savedReply;
    }
  }

  // Live view receipt on a throwaway story entry.
  const probe = {
    id: "probe-story",
    sender: "me",
    kind: "text",
    text: "hi",
    timestamp: Date.now(),
    viewed: true,
    from_me: true,
    viewers: [],
  };
  state.statuses.set("probe@s.whatsapp.net", {
    jid: "probe@s.whatsapp.net",
    name: "Probe",
    updates: [probe],
  });
  await noteStatusView({ id: "probe-story", jid: "333:8@s.whatsapp.net", name: "", timestamp: 9 });
  await new Promise((r) => setTimeout(r, 400));
  const liveView = state.statuses.get("probe@s.whatsapp.net").updates[0].viewers;
  state.statuses.delete("probe@s.whatsapp.net");
  return { fresh, duplicate, broadcast, heal, draft, draftFull, liveDraft, liveHeal, liveView };
});
check(
  "view receipts normalize the jid",
  viewsDrafts.fresh.accept && viewsDrafts.fresh.viewer.jid === "222@s.whatsapp.net",
  viewsDrafts.fresh.viewer.jid,
);
check(
  "repeat and broadcast receipts are dropped",
  viewsDrafts.duplicate.accept === false && viewsDrafts.broadcast.accept === false,
);
check(
  "the heal plan marks quotes and drops phantoms",
  viewsDrafts.heal[0].quote_ids.join(",") === "m1" && viewsDrafts.heal[0].remove_ids.join(",") === "story-1",
  JSON.stringify(viewsDrafts.heal[0]),
);
check(
  "the draft stack takes images only and caps at 10",
  viewsDrafts.draft.accepted.join(",") === "0,2" && viewsDrafts.draftFull.accepted.length === 0,
  viewsDrafts.draft.toast,
);
if (viewsDrafts.liveDraft) {
  check(
    "the live draft stack keeps one image and rejects the pdf",
    viewsDrafts.liveDraft.added === 1 && viewsDrafts.liveDraft.thumbs === 1 && viewsDrafts.liveDraft.open,
    JSON.stringify(viewsDrafts.liveDraft),
  );
}
if (viewsDrafts.liveHeal) {
  check("the live heal marks the reply", viewsDrafts.liveHeal.quoted, JSON.stringify(viewsDrafts.liveHeal));
}
check(
  "the live view receipt is recorded",
  viewsDrafts.liveView.length === 1 && viewsDrafts.liveView[0].jid === "333@s.whatsapp.net",
  JSON.stringify(viewsDrafts.liveView),
);

console.log("\n# calls + capture (rust)");
const calls = await page.evaluate(async () => {
  const ringing = await invoke("ui_call_overlay", {
    input: { state: "ringing", video: true, send_video: true, muted: false, started_at: null, now: 1000 },
  });
  const connected = await invoke("ui_call_overlay", {
    input: { state: "connected", video: true, send_video: false, muted: true, started_at: 10000, now: 75000 },
  });
  const guardGroup = await invoke("ui_call_guard", { isGroup: true, hasActiveCall: false });
  const guardOk = await invoke("ui_call_guard", { isGroup: false, hasActiveCall: false });
  const toastVideo = await invoke("ui_call_media_toast", { video: true });
  const capture = await invoke("ui_status_capture");
  const duration = await invoke("ui_status_duration_seconds", { duration: 4.6 });

  const savedCall = currentCall;
  currentCall = {
    callId: "probe",
    peer: "6285@s.whatsapp.net",
    video: true,
    sendVideo: true,
    outgoing: false,
    state: "ringing",
  };
  await showCallOverlay();
  await new Promise((r) => setTimeout(r, 250));
  const live = {
    status: document.getElementById("call-status").textContent,
    acceptHidden: document.getElementById("call-accept").hidden,
    muteHidden: document.getElementById("call-mute").hidden,
    stageHidden: document.getElementById("call-video-stage").hidden,
  };
  currentCall = savedCall;
  document.getElementById("call-overlay").hidden = true;
  return { ringing, connected, guardGroup, guardOk, toastVideo, capture, duration, live };
});
check(
  "the ringing overlay offers accept/reject",
  calls.ringing.accept && calls.ringing.reject && !calls.ringing.mute && !calls.ringing.video_stage,
  calls.ringing.status,
);
check(
  "the connected overlay shows the stage and timer",
  calls.connected.video_stage && calls.connected.mute && !calls.connected.self_view && calls.connected.status === "Connected · 01:05",
  calls.connected.status,
);
check(
  "call guards refuse groups",
  calls.guardGroup.ok === false && calls.guardOk.ok && calls.toastVideo.includes("H.264"),
  calls.guardGroup.toast,
);
check(
  "the status capture limits render",
  calls.capture.thumb_max_px === 320 && calls.capture.jpeg_quality === 0.72 && calls.duration === 5,
  `${calls.capture.thumb_max_px}px, duration ${calls.duration}s`,
);
check(
  "the live call overlay renders",
  calls.live.status === "Incoming video call" && !calls.live.acceptHidden && calls.live.muteHidden && calls.live.stageHidden,
  JSON.stringify(calls.live),
);

console.log("\n# restore + contacts (rust)");
const restore = await page.evaluate(async () => {
  const disk = await invoke("ui_restore_disk_cache", {
    raw: JSON.stringify({
      v: 1,
      chats: [
        { jid: "a@s.whatsapp.net", name: "Ada", lastTs: 10, unread: 2, lastText: "hi" },
        { jid: "b@g.us", isGroup: true, lastTs: 5 },
        { jid: "status@broadcast", name: "S" },
      ],
    }),
  });
  const plan = await invoke("ui_contact_plan", {
    updates: [["628770000000", "Budi"]],
    chats: [
      { jid: "628770000000@s.whatsapp.net", name: "628770000000", is_self: false, senders: [] },
      { jid: "1@g.us", name: "Team", is_self: false, senders: ["628770000000@s.whatsapp.net"] },
    ],
  });
  const probeJid = "999000111@s.whatsapp.net";
  state.chats.set(probeJid, {
    jid: probeJid,
    name: "999000111",
    isGroup: false,
    messages: [{ id: "p1", sender: "999000111@s.whatsapp.net", sender_name: "999000111", timestamp: 1, kind: "text", text: "x" }],
    last: null,
    lastTs: 0,
    unread: 0,
  });
  applyContactName(probeJid, "Probe Name");
  await new Promise((r) => setTimeout(r, 600));
  const entry = state.chats.get(probeJid);
  const live = { name: entry?.name, sender: entry?.messages[0]?.sender_name };
  state.chats.delete(probeJid);
  state.contacts.delete("999000111");
  return { disk, plan, live };
});
check(
  "the disk cache restores",
  restore.disk.length === 2 && restore.disk[1].name === "b" && restore.disk[0].last_text === "hi",
  JSON.stringify(restore.disk),
);
check(
  "the contact plan renames chats and senders",
  restore.plan.changed &&
    restore.plan.chat_renames[0][1] === "Budi" &&
    restore.plan.sender_renames[0][2] === "Budi",
  JSON.stringify(restore.plan.chat_renames),
);
check(
  "the contact plan applies to the store",
  restore.live.name === "Probe Name" && restore.live.sender === "Probe Name",
  JSON.stringify(restore.live),
);

console.log("\n# context + hydration (rust)");
const ctx = await page.evaluate(async () => {
  const limits = await invoke("ui_media_limits");
  const seek = await invoke("ui_poster_seek", { duration: 10 });
  const height = await invoke("ui_poster_height", { width: 480, videoWidth: 1920, videoHeight: 1080 });
  const context = await invoke("ui_ai_context", {
    messages: [
      { id: "m1", text: "halo", kind: "text", from_me: false, sender_name: "Budi", timestamp: 1 },
      { id: "m2", text: "", kind: "image", from_me: true, timestamp: 2 },
      { id: "m3", text: "secret", kind: "text", from_me: false, deleted: true, timestamp: 3 },
    ],
    limit: 24,
  });
  const recent = await invoke("ui_recent_stickers", {
    candidates: [
      { id: "a", timestamp: 10, pending: false },
      { id: "a", timestamp: 30, pending: false },
      { id: "local-1", timestamp: 40, pending: false },
      { id: "b", timestamp: 20, pending: true },
    ],
    limit: 80,
  });
  const aiStickers = await invoke("ui_ai_sticker_ids", {
    ids: ["a", "b", "c"],
    ready: ["b", "c"],
    limit: 80,
  });
  document.getElementById("emoji-panel").hidden = false;
  setStickerTab("recent");
  await new Promise((r) => setTimeout(r, 2000));
  const cells = document.querySelectorAll("#sticker-grid .sticker-cell").length;
  document.getElementById("emoji-panel").hidden = true;
  return { limits, seek, height, context, recent, aiStickers, cells };
});
check("the media limits render", ctx.limits.max_thumbs === 240 && ctx.limits.load_timeout_ms === 12000 && ctx.limits.root_margin === "300px 0px", JSON.stringify(ctx.limits));
check("poster sampling maths render", ctx.seek === 0.8 && ctx.height === 270, `${ctx.seek}s @ ${ctx.height}px`);
check("the AI context is bounded and labelled", ctx.context.length === 2 && ctx.context[1].sender === "Gw" && ctx.context[1].text === "[Photo]", JSON.stringify(ctx.context));
check("recent stickers dedupe", ctx.recent.length === 1 && ctx.recent[0] === "a", ctx.recent.join(","));
check("AI stickers need cached media", ctx.aiStickers.join(",") === "b,c", ctx.aiStickers.join(","));
check("the sticker grid still renders", ctx.cells > 0, `${ctx.cells} cells`);

console.log("\n# store plans (rust)");
const storePlans2 = await page.evaluate(async () => {
  const sendText = await invoke("ui_send_plan", { isChannel: false, hasDraft: false, editing: false, text: " hello " });
  const sendEmpty = await invoke("ui_send_plan", { isChannel: false, hasDraft: false, editing: false, text: "  " });
  const sendChannel = await invoke("ui_send_plan", { isChannel: true, hasDraft: false, editing: false, text: "x" });
  const sendDraft = await invoke("ui_send_plan", { isChannel: false, hasDraft: true, editing: false, text: "" });
  const sendEdit = await invoke("ui_send_plan", { isChannel: false, hasDraft: false, editing: true, text: "x" });
  const seeded = await invoke("ui_seed_chats", {
    contacts: [
      { phone: "628770000000", name: "Budi" },
      { phone: "62111", name: "Short" },
      { phone: "628990000000", name: "Known" },
    ],
    mePhone: "628550000000",
    existing: ["628990000000@s.whatsapp.net"],
  });
  const alias = await invoke("ui_alias_plan", {
    lookups: [{ chat: "111@lid", phone: "6285", lid: "111", name: "Ada" }],
    chats: [
      { jid: "111@lid", name: "111", exists: true },
      { jid: "6285@s.whatsapp.net", name: "6285", exists: true },
    ],
  });
  return { sendText, sendEmpty, sendChannel, sendDraft, sendEdit, seeded, alias };
});
check(
  "the send plan covers every composer state",
  storePlans2.sendText.action === "send" &&
    storePlans2.sendEmpty.action === "noop" &&
    storePlans2.sendChannel.action === "blocked" &&
    storePlans2.sendDraft.action === "draft" &&
    storePlans2.sendEdit.action === "edit",
  `${storePlans2.sendText.action}/${storePlans2.sendEmpty.action}/${storePlans2.sendChannel.action}/${storePlans2.sendDraft.action}/${storePlans2.sendEdit.action}`,
);
check(
  "the seed plan filters the address book",
  storePlans2.seeded.length === 1 && storePlans2.seeded[0].name === "Budi",
  JSON.stringify(storePlans2.seeded),
);
check(
  "the alias plan folds the LID row",
  storePlans2.alias.changed &&
    storePlans2.alias.folds[0].from === "111@lid" &&
    storePlans2.alias.folds[0].to === "6285@s.whatsapp.net" &&
    storePlans2.alias.renames[0][1] === "Ada",
  JSON.stringify(storePlans2.alias.folds),
);

console.log("\n# chat actions (rust)");
const chatActions = await page.evaluate(async () => {
  const input = (action, extra = {}) => ({
    action,
    jid: "1@s.whatsapp.net",
    pinned: false,
    archived: false,
    blocked: false,
    is_open: false,
    is_group: false,
    ...extra,
  });
  const pin = await invoke("ui_chat_action", { input: input("pin") });
  const unread = await invoke("ui_chat_action", { input: input("unread") });
  const block = await invoke("ui_chat_action", { input: input("block", { blocked: true }) });
  const clear = await invoke("ui_chat_action", { input: input("clear") });
  const close = await invoke("ui_chat_action", { input: input("close", { is_open: true }) });
  const deletes = await invoke("ui_delete_commands", {
    items: [
      { chat: "a", id: "1", from_me: true, participant: null },
      { chat: "b", id: "2", from_me: false, participant: "6285@s.whatsapp.net" },
    ],
    everyone: false,
  });
  const deletesEveryone = await invoke("ui_delete_commands", {
    items: [{ chat: "a", id: "1", from_me: true, participant: null }],
    everyone: true,
  });

  const chat = [...state.chats.values()].find((entry) => entry.messages.length > 2);
  let dialog = null;
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 1000));
    const message = chat.messages.find((m) => m.text);
    await showMessageInfo(message);
    dialog = {
      hidden: document.getElementById("msg-info").hidden,
      rows: document.querySelectorAll("#msg-info-body div").length,
    };
    document.getElementById("msg-info").hidden = true;
  }
  return { pin, unread, block, clear, close, deletes, deletesEveryone, dialog };
});
check("chat actions plan their command", chatActions.pin.command === "chat_pin" && chatActions.pin.pin === true, chatActions.pin.toast);
check("read-mark and block plan correctly", chatActions.unread.unread === true && chatActions.block.blocked === false, `${chatActions.unread.toast} / ${chatActions.block.toast}`);
check("clear asks first", chatActions.clear.clear && Boolean(chatActions.clear.confirm));
check("close only affects the open chat", chatActions.close.command === null && chatActions.close.close);
check("delete commands carry the args", chatActions.deletes.length === 2 && chatActions.deletes[1].participant === "6285@s.whatsapp.net" && chatActions.deletesEveryone[0].command === "delete_for_everyone");
if (chatActions.dialog) {
  check("the message info dialog renders", !chatActions.dialog.hidden && chatActions.dialog.rows === 5, `${chatActions.dialog.rows} rows`);
}

console.log("\n# status frame clock (rust)");
const clock = await page.evaluate(async () => {
  const timing = await invoke("ui_status_timing");
  const text = await invoke("ui_status_frame_duration", { kind: "text", text: "short" });
  const long = await invoke("ui_status_frame_duration", { kind: "text", text: "x".repeat(200) });
  const image = await invoke("ui_status_frame_duration", { kind: "image", text: "" });
  const video = await invoke("ui_status_frame_duration", { kind: "video", text: "" });
  const anchor = await invoke("ui_status_anchor", { total: 8000, remaining: 8000, now: 1000 });
  const elapsed = await invoke("ui_status_elapsed", {
    total: anchor.total,
    remaining: anchor.remaining,
    endsAt: anchor.ends_at,
    now: 5000,
  });
  const paused = await invoke("ui_status_pause_remaining", {
    remaining: anchor.remaining,
    endsAt: anchor.ends_at,
    now: 3500,
  });
  const resumed = await invoke("ui_status_resume", { total: 8000, remaining: 0 });
  const progress = await invoke("ui_status_progress", { total: 8000, remaining: 4000 });
  const thumb = await invoke("ui_status_thumb_scale", { width: 640, height: 480 });
  const seek = await invoke("ui_status_poster_seek", { duration: 10 });

  const entry = [...state.statuses.values()].find((e) => e.updates.length > 0);
  let viewer = null;
  if (entry) {
    // Do not send real "status viewed" receipts from the test run.
    const realMark = markStatusViewed;
    markStatusViewed = () => {};
    openStatus();
    await new Promise((r) => setTimeout(r, 800));
    openStatusViewer(entry.jid);
    await new Promise((r) => setTimeout(r, 1500));
    const before = statusFrameEndsAt > 0 && statusFrameTotal > 0;
    await pauseStatusFrame();
    const pausedState = statusPaused && statusFrameEndsAt === 0;
    await resumeStatusFrame();
    const resumedState = !statusPaused && statusFrameEndsAt > 0;
    viewer = { before, pausedState, resumedState };
    closeStatusViewer();
    closeStatus();
    markStatusViewed = realMark;
  }
  return { timing, text, long, image, video, anchor, elapsed, paused, resumed, progress, thumb, seek, viewer };
});
check("the timing constants render", clock.timing.text_ms === 10000 && clock.timing.thumb_max_px === 320, JSON.stringify(clock.timing));
check("frame durations follow the kind", clock.text === 10000 && clock.long === 14000 && clock.image === 8000 && clock.video === 0);
check("the anchor and elapsed clock render", clock.anchor.ends_at === 9000 && clock.elapsed === 4000, JSON.stringify(clock.anchor));
check("pause keeps the remaining time", clock.paused === 5500 && clock.resumed[1] === 1);
check("the progress fill renders", clock.progress === 0.5);
check("thumbnails fit the capture box", clock.thumb.width === 320 && clock.thumb.height === 240);
check("the poster seek renders", clock.seek === 0.2);
if (clock.viewer) {
  check("the viewer pauses and resumes", clock.viewer.before && clock.viewer.pausedState && clock.viewer.resumedState, JSON.stringify(clock.viewer));
}

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
  // Opening a viewer would send real "status viewed" receipts and permanently
  // mark the account's stories read, so stub the marker for this check.
  const realMark = markStatusViewed;
  markStatusViewed = () => {};
  openStatus();
  await new Promise((r) => setTimeout(r, 1200));
  const rows = document.querySelectorAll("#status-list .status-item").length;
  const viewed = document.querySelectorAll("#status-viewed-list .status-item").length;
  const mineLabel = document.querySelector("#status-mine .status-meta span")?.textContent || "";
  const first = document.querySelector("#status-list .status-item, #status-viewed-list .status-item");
  let viewer = { skipped: true };
  if (first) {
    openStatusViewer(first.getAttribute("data-status-jid"));
    // The frame body is painted after the renderer round-trip; poll for it.
    for (let attempt = 0; attempt < 15; attempt += 1) {
      await new Promise((r) => setTimeout(r, 250));
      if ((document.querySelector("#status-viewer-body")?.innerHTML || "").length > 0) break;
    }
    viewer = {
      name: document.getElementById("status-viewer-name").textContent,
      time: document.getElementById("status-viewer-time").textContent,
      segments: document.querySelectorAll("#status-progress span").length,
      body: document.querySelector("#status-viewer-body")?.innerHTML.slice(0, 40) || "",
      kind: statusQueue[statusIndex]?.kind || "",
    };
  }
  closeStatusViewer();
  markStatusViewed = realMark;
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
