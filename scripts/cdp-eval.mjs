/* Tiny CDP client: run JS in the WA Desk webview and print the result.
   Usage: node scripts/cdp-eval.mjs '<expression>' [click x y]
          node scripts/cdp-eval.mjs --file <path.js> [click x y]
   The --file form keeps long snippets free of shell quoting. */
import fs from "node:fs";

const fileMode = process.argv[2] === "--file";
const expr = fileMode
  ? fs.readFileSync(process.argv[3], "utf8")
  : process.argv[2];
if (!expr) {
  console.error("missing expression");
  process.exit(1);
}

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

const result = await send("Runtime.evaluate", {
  expression: expr,
  awaitPromise: true,
  returnByValue: true,
});
if (result.exceptionDetails) {
  console.error("exception", JSON.stringify(result.exceptionDetails, null, 2));
} else {
  console.log(JSON.stringify(result.result.value, null, 2));
}

// Real (trusted) click when coordinates are given, so audio autoplay counts
// as a user gesture like an actual human click.
const [cx, cy] = process.argv.slice(fileMode ? 4 : 3).map(Number);
if (Number.isFinite(cx) && Number.isFinite(cy)) {
  await send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: cx,
    y: cy,
    button: "none",
  });
  await send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: cx,
    y: cy,
    button: "left",
    buttons: 1,
    clickCount: 1,
  });
  await send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: cx,
    y: cy,
    button: "left",
    buttons: 0,
    clickCount: 1,
  });
}
ws.close();
