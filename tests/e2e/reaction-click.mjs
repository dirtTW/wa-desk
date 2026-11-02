import { chromium } from "playwright-core";

const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
const pages = browser.contexts().flatMap((context) => context.pages());
const page = pages.find((p) => p.url().startsWith("http://tauri.localhost")) || pages[0];
if (!page) throw new Error("no WA Desk page");
await page.waitForSelector("#rail");

// Open a chat that has a text message.
await page.evaluate(() => {
  const chat = [...state.chats.values()].find((c) => c.messages.some((m) => m.kind === "text" && !m.deleted));
  if (chat) openChat(chat.jid);
});
await page.waitForTimeout(400);

// Long-press the first bubble without touching the real cursor.
await page.evaluate(() => {
  const bubble = document.querySelector("#messages .bubble");
  const rect = bubble.getBoundingClientRect();
  bubble.dispatchEvent(
    new PointerEvent("pointerdown", {
      bubbles: true,
      clientX: rect.x + 20,
      clientY: rect.y + 12,
      pointerId: 1,
      pointerType: "mouse",
    }),
  );
});
await page.waitForTimeout(600);
await page.evaluate(() =>
  document.querySelector("#messages .bubble")?.dispatchEvent(
    new PointerEvent("pointerup", { bubbles: true, pointerId: 1, pointerType: "mouse" }),
  ),
);
await page.waitForTimeout(300);
console.log("menu open:", await page.locator("#msg-menu").isVisible());

const target = await page.evaluate(() => {
  const bubble = document.querySelector("#messages .bubble");
  return bubble ? bubble.dataset.id : null;
});
const before = await page.evaluate(
  (id) => {
    const chat = [...state.chats.values()].find((c) => c.messages.some((m) => m.id === id));
    const message = chat?.messages.find((m) => m.id === id);
    return message ? JSON.stringify(message.reactions || []) : null;
  },
  target,
);

// Click a quick reaction with a REAL Playwright click.
await page.locator('#reaction-bar button[data-emoji="👍"]').click();
await page.waitForTimeout(1200);
const afterQuick = await page.evaluate(
  (id) => {
    const chat = [...state.chats.values()].find((c) => c.messages.some((m) => m.id === id));
    const message = chat?.messages.find((m) => m.id === id);
    return message ? JSON.stringify(message.reactions || []) : null;
  },
  target,
);
console.log("quick reaction:", before, "->", afterQuick, "menu hidden:", await page.locator("#msg-menu").isHidden());

// Re-open and use "+" then a real emoji tile click.
await page.evaluate(() => {
  const bubble = document.querySelector("#messages .bubble");
  const rect = bubble.getBoundingClientRect();
  bubble.dispatchEvent(
    new PointerEvent("pointerdown", {
      bubbles: true,
      clientX: rect.x + 20,
      clientY: rect.y + 12,
      pointerId: 2,
      pointerType: "mouse",
    }),
  );
});
await page.waitForTimeout(600);
await page.evaluate(() =>
  document.querySelector("#messages .bubble")?.dispatchEvent(
    new PointerEvent("pointerup", { bubbles: true, pointerId: 2, pointerType: "mouse" }),
  ),
);
await page.waitForTimeout(300);
await page.click("[data-reaction-more]");
await page.waitForTimeout(400);
const tile = page.locator('#emoji-scroll .emoji[data-char="🐼"]').first();
console.log("picker open:", await page.locator("#emoji-panel").isVisible(), "tile:", await tile.count());
if (await tile.count()) {
  await tile.click();
  await page.waitForTimeout(1200);
}
const afterPlus = await page.evaluate(
  (id) => {
    const chat = [...state.chats.values()].find((c) => c.messages.some((m) => m.id === id));
    const message = chat?.messages.find((m) => m.id === id);
    return message ? JSON.stringify(message.reactions || []) : null;
  },
  target,
);
console.log("plus reaction:", afterQuick, "->", afterPlus, "picker hidden:", await page.locator("#emoji-panel").isHidden());

await browser.close();
