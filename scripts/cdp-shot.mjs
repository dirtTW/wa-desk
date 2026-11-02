/* Tiny CDP client: capture a PNG of the WA Desk webview (optionally clipped).
   Usage: node scripts/cdp-shot.mjs <out.png> [x y w h]   (clip in CSS pixels) */
import fs from "node:fs";

const out = process.argv[2] || "shot.png";
const clip =
  process.argv.length >= 6
    ? {
        x: Number(process.argv[3]),
        y: Number(process.argv[4]),
        width: Number(process.argv[5]),
        height: Number(process.argv[6]),
        scale: 1,
      }
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
  ...(clip ? { clip } : {}),
});
fs.writeFileSync(out, Buffer.from(shot.data, "base64"));
console.log("saved", out);
