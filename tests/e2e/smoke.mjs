import { chromium } from "playwright-core";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const artifacts = path.join(here, "artifacts");
fs.mkdirSync(artifacts, { recursive: true });

let failures = 0;
function check(name, condition, detail = "") {
  if (condition) {
    console.log(`  ok  ${name}${detail ? ` — ${detail}` : ""}`);
  } else {
    failures += 1;
    console.error(`FAIL  ${name}${detail ? ` — ${detail}` : ""}`);
  }
}

const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
const contexts = browser.contexts();
const pages = contexts.flatMap((context) => context.pages());
const page = pages.find((p) => p.url().startsWith("http://tauri.localhost")) || pages[0];
if (!page) throw new Error("WA Desk webview not found on port 9222");
await page.waitForSelector("#rail", { timeout: 10000 });

console.log("\n# shell");
check("window title", (await page.title()) === "WA Desk");
check("left rail", (await page.locator("#rail").count()) === 1);
check("chat list", (await page.locator("#chat-list .chat-item").count()) > 0);

console.log("\n# new chat picker");
await page.click("#btn-compose");
await page.waitForSelector("#new-chat-screen:not([hidden])");
check("compose opens the picker", await page.locator("#new-chat-screen").isVisible());
await page.fill("#new-chat-input", "pak ade");
await page.waitForTimeout(250);
check(
  "search filters saved contacts",
  (await page.locator("#new-chat-list .new-chat-row strong").first().textContent()).toLowerCase().includes("pak ade"),
);
await page.click("#new-chat-back");
check("back closes the picker", await page.locator("#new-chat-screen").isHidden());

console.log("\n# settings");
await page.click("#rail-settings");
await page.waitForSelector("#settings-screen:not([hidden])");
check("settings opens in the left panel", await page.locator("#settings-screen").isVisible());
await page.click('#settings-body [data-page="chats"]');
await page.waitForTimeout(200);
const toggles = await page.locator("#settings-body [data-toggle]").evaluateAll((nodes) => nodes.map((n) => n.dataset.toggle));
check("chat settings toggles", ["notifications", "autostart"].every((t) => toggles.includes(t)), toggles.join(","));
await page.click("#settings-back");
await page.click("#settings-back");
check("settings closes", await page.locator("#settings-screen").isHidden());

console.log("\n# message menu + any-emoji reaction picker");
await page.evaluate(() => {
  const chat = [...state.chats.values()].find((c) => c.messages.some((m) => m.kind === "text" && !m.deleted));
  if (chat) openChat(chat.jid);
});
await page.waitForTimeout(500);
const pressed = await page.evaluate(() => {
  const bubble = document.querySelector("#messages .bubble");
  if (!bubble) return false;
  const rect = bubble.getBoundingClientRect();
  const opts = { bubbles: true, clientX: rect.x + 20, clientY: rect.y + 12, pointerId: 1, pointerType: "mouse" };
  bubble.dispatchEvent(new PointerEvent("pointerdown", opts));
  return true;
});
check("long-press target", pressed);
await page.waitForTimeout(650);
await page.evaluate(() => document.querySelector("#messages .bubble")?.dispatchEvent(new PointerEvent("pointerup", { bubbles: true })));
await page.waitForTimeout(250);
check("long press opens menu", await page.locator("#msg-menu").isVisible());
check("reaction bar", (await page.locator("#reaction-bar button[data-emoji]").count()) === 6);

await page.click("[data-reaction-more]");
await page.waitForTimeout(400);
check("+ opens the full emoji picker", await page.locator("#emoji-panel").isVisible());
const reactionTarget = await page.evaluate(() => Boolean(reactionPickMessage));
check("picker is in reaction mode", reactionTarget);
await page.keyboard.press("Escape");
await page.waitForTimeout(200);
check("picker closes", await page.locator("#emoji-panel").isHidden());

console.log("\n# media viewer");
const media = await page.evaluate(async () => {
  for (const chat of state.chats.values()) {
    for (const message of chat.messages) {
      if (message.kind !== "image" || !WAChatUtils.isMessageVisible(message, false)) continue;
      try {
        const file = await window.__TAURI__.core.invoke("fetch_media", { id: message.id });
        rememberMedia(message.id, window.__TAURI__.core.convertFileSrc(file));
        openChat(chat.jid);
        renderChat();
        return { id: message.id, chat: chat.name };
      } catch {
        /* try the next one */
      }
    }
  }
  return null;
});
check("image available", Boolean(media), media ? `${media.chat}` : "none");
if (media) {
  await page.waitForTimeout(600);
  const image = page.locator(`.media-image[data-media-id="${media.id}"]`);
  check("image rendered", (await image.count()) > 0);
  await image.first().click();
  await page.waitForSelector("#media-viewer:not([hidden])");
  check("viewer opens on click", await page.locator("#media-viewer").isVisible());
  check("stage shows image", (await page.locator("#media-viewer-stage img").count()) === 1);
  check("filmstrip", (await page.locator(".media-strip-thumb").count()) > 0);
  check("download link", Boolean(await page.locator("#media-viewer-download").getAttribute("href")));
  await page.screenshot({ path: path.join(artifacts, "media-viewer.png") });
  await page.click("#media-viewer-close");
  check("viewer closes", await page.locator("#media-viewer").isHidden());
}

console.log("\n# group info + member menu");
const group = await page.evaluate(() => {
  const chat = [...state.chats.values()].find((c) => c.isGroup && c.messages.length);
  if (chat) openChat(chat.jid);
  return chat ? { name: chat.name } : null;
});
if (group) {
  await page.waitForTimeout(500);
  await page.click("#chat-menu");
  await page.waitForSelector("#group-info-screen:not([hidden])");
  await page.waitForTimeout(700);
  const members = await page.locator(".group-member").count();
  check("group info lists members", members > 0, `${group.name}: ${members}`);
  const more = page.locator(".group-member .member-more").first();
  if (await more.count()) {
    await more.click();
    check("member actions menu", await page.locator("#member-menu").isVisible());
    await page.keyboard.press("Escape");
  }
  await page.click("#group-info-back");
}

await browser.close();
console.log(`\n${failures === 0 ? "PASS" : "FAIL"} — ${failures} failure(s)`);
process.exit(failures === 0 ? 0 : 1);
