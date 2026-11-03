import { chromium } from "playwright-core";

const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
const page = browser.contexts().flatMap((c) => c.pages())[0];
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
});

const result = await page.evaluate(async () => {
  const out = {};

  // Media viewer: open the first chat that has a photo or video.
  const chat = [...state.chats.values()].find((entry) =>
    entry.messages.some((m) => m.kind === "image" || m.kind === "video"),
  );
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 1200));
    const media = chat.messages.find((m) => m.kind === "image" || m.kind === "video");
    openMediaViewer(media.id);
    await new Promise((r) => setTimeout(r, 2500));
    out.viewer = {
      hidden: document.getElementById("media-viewer").hidden,
      chat: document.getElementById("media-viewer-chat").textContent,
      time: document.getElementById("media-viewer-time").textContent,
      stage: document.getElementById("media-viewer-stage").innerHTML.slice(0, 50),
      strip: document.querySelectorAll("#media-viewer-strip .media-strip-thumb").length,
      active: document.querySelectorAll("#media-viewer-strip .active").length,
      prevDisabled: document.getElementById("media-viewer-prev").disabled,
      kind: media.kind,
    };
    closeMediaViewer();
  }

  // Sticker grid: open the picker on the stickers tab.
  if (chat) {
    await openChat(chat.jid);
    await new Promise((r) => setTimeout(r, 800));
  }
  document.getElementById("emoji-panel").hidden = false;
  setStickerTab("recent");
  await new Promise((r) => setTimeout(r, 2500));
  out.stickers = {
    create: document.querySelectorAll("#sticker-grid .sticker-create").length,
    cells: document.querySelectorAll("#sticker-grid .sticker-cell").length,
    images: document.querySelectorAll("#sticker-grid .sticker-cell img").length,
    count: document.getElementById("sticker-count").textContent,
  };
  document.getElementById("emoji-panel").hidden = true;
  return out;
});

console.log(JSON.stringify(result, null, 1));
console.log("errors", errors.slice(0, 3));
await page.screenshot({ path: "tests/e2e/artifacts/viewer-rust.png" });
process.exit(0);
