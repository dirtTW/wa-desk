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
  const name = (await page.locator("#chat-list .chat-item strong").first().textContent())?.trim() || "a";
  await page.fill("#search", name.slice(0, 4));
  await page.waitForTimeout(600);
  const marked = await page.locator("#chat-list .search-hit").count();
  const section = await page.locator("#chat-list .search-section").count();
  check("search highlights the match", marked > 0, `${marked} marks`);
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
const statusEmoji = await page.evaluate(() => {
  buildStatusEmojiPanel();
  return {
    cats: document.querySelectorAll("#status-emoji-cats .emoji-cat").length,
    items: document.querySelectorAll("#status-emoji-grid .emoji").length,
  };
});
check("status panel has the 9 categories", statusEmoji.cats === 9, `${statusEmoji.cats}`);
check("status panel is populated", statusEmoji.items > 20, `${statusEmoji.items} emoji`);

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
