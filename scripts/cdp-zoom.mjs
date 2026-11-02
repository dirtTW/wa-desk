/* Tiny CDP client: capture a magnified PNG of one region of the WA Desk
   webview, so a 3 px progress bar can be inspected at pixel level.
   Usage: node scripts/cdp-zoom.mjs <out.png> [x y w h] [scale] */
import fs from "node:fs";

const out = process.argv[2] || "zoom.png";
const nums = process.argv.slice(3).map(Number);
const scale = Number.isFinite(nums[4]) ? nums[4] : 6;
const clip =
  nums.length >= 4
    ? { x: nums[0], y: nums[1], width: nums[2], height: nums[3], scale }
    : undefined;

const tabs = await (await fetch("http://127.0.0.1:9224/json/list")).json();
const page = tabs.find((tab) => tab.type === "page");
if (!page) {
  console.error("WA Desk page not found:", tabs.map((t) => t.url));
  process.exit(1);
}

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = reject;
});

let id = 0;
const pending = new Map();
function send(method, params = {}) {
  const callId = ++id;
  ws.send(JSON.stringify({ id: callId, method, params }));
  return new Promise((resolve, reject) => {
    pending.set(callId, { resolve, reject });
  });
}

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
  }
};

const shot = await send("Page.captureScreenshot", {
  format: "png",
  captureBeyondViewport: false,
  ...(clip ? { clip } : {}),
});
fs.writeFileSync(out, Buffer.from(shot.data, "base64"));
console.log(`saved ${out} (${clip ? `${clip.width}x${clip.height} @${scale}x` : "viewport"})`);
ws.close();
process.exit(0);
