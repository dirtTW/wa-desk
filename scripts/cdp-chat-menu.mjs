/* Verify the official chat-menu flow (right-click → rows → mute submenu)
   against the live WA Desk release instance, without performing a mute. */
const tabs = await (await fetch("http://127.0.0.1:9224/json/list")).json();
const page = tabs.find((tab) => tab.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, err) => { ws.onopen = ok; ws.onerror = err; });
let id = 0;
const pend = new Map();
const send = (method, params = {}) => {
  const i = ++id;
  ws.send(JSON.stringify({ id: i, method, params }));
  return new Promise((res, rej) => pend.set(i, { res, rej }));
};
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pend.has(m.id)) { pend.get(m.id).res(m.result); pend.delete(m.id); }
};
const evalJs = async (expr) => {
  const r = await send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || "eval failed");
  return r.result.value;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const click = async (x, y, button = "left") => {
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "none" });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, buttons: button === "right" ? 2 : 1, clickCount: 1 });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, buttons: button === "right" ? 0 : 0, clickCount: 1 });
  await sleep(250);
};

const spot = await evalJs(`(()=>{const d=[...document.querySelectorAll('.chat-item')].find(r=>r.textContent.includes('Alice'));
const r=d.getBoundingClientRect();return JSON.stringify({x:Math.round(r.x+r.width/2),y:Math.round(r.y+18)})})()`);
const { x, y } = JSON.parse(spot);
await click(x, y, "right");
const rows = await evalJs(`(()=>{const m=document.getElementById('chat-list-menu');
return JSON.stringify([...m.querySelectorAll('button[data-chat-action]')].filter(b=>!b.hidden).map(b=>b.textContent.trim()))})()`);
// Open the mute submenu without choosing an option.
const muteRow = await evalJs(`(()=>{const m=document.getElementById('chat-list-menu');
const b=[...m.querySelectorAll('button[data-chat-action]')].find(b=>b.dataset.chatAction==='mute');
const r=b.getBoundingClientRect();return JSON.stringify({x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2)})})()`);
const mp = JSON.parse(muteRow);
await click(mp.x, mp.y);
await sleep(400);
const sub = await evalJs(`(()=>{const s=document.getElementById('chat-mute-sub');
return JSON.stringify({hidden:s.hidden, rows:[...s.querySelectorAll('button[data-mute-choice]')].map(b=>b.textContent.trim())})})()`);
console.log(JSON.stringify({ menuRows: JSON.parse(rows), muteSubmenu: JSON.parse(sub) }, null, 1));
ws.close();
