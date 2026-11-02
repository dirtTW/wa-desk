/* Full audio playback flow against the live WA Desk webview:
   play -> verify -> seek mid -> verify -> pause -> verify. */
const tabs = await (await fetch("http://127.0.0.1:9224/json/list")).json();
const page = tabs.find((tab) => tab.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, err) => { ws.onopen = ok; ws.onerror = err; });
let id = 0;
const pend = new Map();
const send = (m, p) => new Promise((res, rej) => {
  const i = ++id;
  ws.send(JSON.stringify({ id: i, method: m, params: p || {} }));
  pend.set(i, { res, rej });
});
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pend.has(m.id)) { pend.get(m.id).res(m.result); pend.delete(m.id); }
};
const evalJs = async (expr) => {
  const r = await send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails.exception?.description || r.exceptionDetails));
  return r.result.value;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const click = async (x, y) => {
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "none" });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: 1 });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: 1 });
  await sleep(150);
};
const stateOf = () => evalJs(`(()=>{
  const p=document.querySelector('.audio-player'); if(!p) return null;
  const el=p.querySelector('audio.audio-src');
  const rc=(n)=>n.getBoundingClientRect();
  const r=rc(p.querySelector('.audio-play'));
  return {paused:el.paused,time:+el.currentTime.toFixed(2),playing:p.classList.contains('playing'),
    fill:p.querySelector('[data-audio-fill]').style.width,label:p.querySelector('[data-audio-dur]').textContent,
    btn:{x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2)},
    trackX:Math.round(rc(p.querySelector('.audio-track')).left),trackW:Math.round(rc(p.querySelector('.audio-track')).width)};
})()`);

// Rewind so every run starts from the beginning of the clip.
await evalJs(`(()=>{const el=document.querySelector('.bubble.voice audio.audio-src'); if(el){el.currentTime=0;el.pause();} return 1;})()`);
await sleep(300);

let s = await stateOf();
// First trusted click: play.
await click(s.btn.x, s.btn.y);
await sleep(1800);
const playing = await stateOf();
await click(Math.round(playing.trackX + playing.trackW * 0.75), playing.btn.y); // seek to the mid-right
await sleep(700);
const seeked = await stateOf();
await click(seeked.btn.x, seeked.btn.y); // pause
await sleep(500);
const paused = await stateOf();
console.log(JSON.stringify({ playing, seeked, paused }, null, 1));
ws.close();
