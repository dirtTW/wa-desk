/* Capture the status progress-bar strip and report the real pixel colours,
   anchored to the segment geometry captured in the SAME layout state and scaled
   by the device pixel ratio (Windows display scaling made earlier samples lie).
   Usage: node scripts/cdp-bars.mjs [samples]  */
const samples = Number(process.argv[2] || 1);

const tabs = await (await fetch("http://127.0.0.1:9224/json/list")).json();
const page = tabs.find((tab) => tab.type === "page");
if (!page) {
  console.error("WA Desk page not found");
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
  return new Promise((resolve, reject) => pending.set(callId, { resolve, reject }));
}
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
  }
};
async function evaluate(expression) {
  const result = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
}

const LAYOUT = `(() => {
  const prog = document.getElementById("status-progress");
  const r = prog.getBoundingClientRect();
  const kids = [...prog.children];
  const segs = kids.map((s) => {
    const b = s.getBoundingClientRect();
    const fill = s.querySelector("i");
    return {
      x: Math.round((b.x - r.x) * 100) / 100,
      w: Math.round(b.width * 100) / 100,
      cls: s.className,
      fillW: Math.round((fill?.getBoundingClientRect().width || 0) * 100) / 100,
    };
  });
  return {
    key: kids.length + ":" + statusIndex + ":" + statusQueue.length + ":" +
      segs.map((s) => s.x + "-" + s.w).join("|"),
    dpr: window.devicePixelRatio,
    x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height),
    barTop: Math.round((kids[0]?.getBoundingClientRect().y || 0) - r.y),
    segs,
    frame: { index: statusIndex, kind: statusQueue[statusIndex]?.kind, paused: statusPaused },
  };
})()`;

const out = [];
for (let n = 0; n < samples; n += 1) {
  let before = null;
  let after = null;
  let shot = null;
  for (let attempt = 0; attempt < 6; attempt += 1) {
    before = await evaluate(LAYOUT);
    shot = await send("Page.captureScreenshot", {
      format: "png",
      captureBeyondViewport: false,
      clip: { x: before.x, y: before.y, width: before.w, height: Math.max(4, before.h), scale: 1 },
    });
    after = await evaluate(LAYOUT);
    if (after.key === before.key) break;
  }
  if (after.key !== before.key) {
    out.push({ skipped: "layout changed during capture" });
    continue;
  }
  const decoded = await evaluate(`(async () => {
    const img = new Image();
    img.src = "data:image/png;base64,${shot.data}";
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.width; c.height = img.height;
    const g = c.getContext("2d");
    g.drawImage(img, 0, 0);
    const s = img.width / ${before.w}; // real raster scale of this capture
    const row = Math.round((${before.barTop} + 1) * s);
    const data = g.getImageData(0, row, img.width, 1).data;
    const px = (x) => {
      const o = Math.max(0, Math.min(img.width - 1, x)) * 4;
      return data[o] + "," + data[o + 1] + "," + data[o + 2];
    };
    const runs = [];
    let prev = null;
    for (let x = 0; x < img.width; x += 1) {
      const key = px(x);
      if (prev && prev.key === key) prev.end = x;
      else { prev = { key, start: x, end: x }; runs.push(prev); }
    }
    const segs = ${JSON.stringify(before.segs)};
    // Column classification in RASTER coordinates: inside a segment (2 px inset
    // from each end) or in the gap between segments.
    const kindOf = (x) => {
      for (let i = 0; i < segs.length; i += 1) {
        const a = segs[i].x * s + 2;
        const b = (segs[i].x + segs[i].w) * s - 2;
        if (x >= a && x <= b) return "seg" + i;
      }
      return "gap";
    };
    const darkByKind = {};
    const blackRuns = [];
    for (const run of runs) {
      const [r, gg, b] = run.key.split(",").map(Number);
      if (r + gg + b > 60) continue;
      blackRuns.push({ start: run.start, end: run.end, w: run.end - run.start + 1, where: kindOf(run.start) });
      for (let x = run.start; x <= run.end; x += 1) {
        const k = kindOf(x);
        darkByKind[k] = (darkByKind[k] || 0) + 1;
      }
    }
    return {
      rasterScale: s,
      segPixels: segs.map((sg, i) => ({
        i, cls: sg.cls,
        centre: px(Math.round((sg.x + sg.w / 2) * s)),
        startOfFill: px(Math.round((sg.x + sg.fillW / 2) * s)),
      })),
      // Painted white width per segment (raster px): the visible fill, which is
      // what "smooth" is judged on — not the layout width.
      paintedFill: segs.map((sg) => {
        const a = Math.round(sg.x * s);
        const b = Math.round((sg.x + sg.w) * s);
        let n = 0;
        for (let x = a; x < b; x += 1) {
          const [r, gg, bl] = px(x).split(",").map(Number);
          if (r + gg + bl > 600) n += 1;
        }
        return n;
      }),
      blackRuns,
      darkByKind,
    };
  })()`);
  out.push({ frame: before.frame, segs: before.segs.map((s) => ({ x: s.x, w: s.w, cls: s.cls, fillW: s.fillW })), ...decoded });
}

console.log(JSON.stringify({ samples: out }, null, 1));
ws.close();
process.exit(0);
