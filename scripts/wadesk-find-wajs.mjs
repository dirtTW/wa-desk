// Find the WhatsApp Web JS source for app-state crypto in the official app's
// WebView2 caches (raw, gzip, or brotli bodies).
import fs from "node:fs";
import zlib from "node:zlib";

const root =
  process.argv[3] ||
  `${process.env.LOCALAPPDATA || `${process.env.USERPROFILE}/AppData/Local`}/Packages/5319275A.WhatsAppDesktop_cv1g1gvanyjgm/LocalCache/EBWebView/Default`;
const needles = ["WhatsApp Patch Integrity", "subtractThenAdd", "slice(-32)", "decodeSyncdSnapshot"];
const outDir = process.argv[2] || ".";
let scanned = 0;

function tryDecode(buf) {
  const cands = [buf];
  if (buf.length > 2 && buf[0] === 0x1f && buf[1] === 0x8b) {
    try {
      cands.push(zlib.gunzipSync(buf));
    } catch {}
  }
  try {
    cands.push(zlib.brotliDecompressSync(buf));
  } catch {}
  return cands;
}

function walk(d) {
  let ents;
  try {
    ents = fs.readdirSync(d, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of ents) {
    const p = d + "/" + e.name;
    if (e.isDirectory()) {
      walk(p);
      continue;
    }
    let st;
    try {
      st = fs.statSync(p);
    } catch {
      continue;
    }
    if (st.size > 80 * 1024 * 1024 || st.size < 100) continue;
    let buf;
    try {
      buf = fs.readFileSync(p);
    } catch {
      continue;
    }
    scanned++;
    for (const cand of tryDecode(buf)) {
      const s = cand.toString("latin1");
      for (const n of needles) {
        const i = s.indexOf(n);
        if (i >= 0) {
          const rel = p.replace(root, "").replace(/[\\/]/g, "_");
          const name = `${outDir}/js-${rel}-${n.replace(/[^a-z0-9]/gi, "")}.txt`;
          fs.writeFileSync(name, cand);
          console.log(`FOUND [${n}] ${p} (decoded ${cand.length} bytes) -> ${name}`);
          break;
        }
      }
    }
  }
}
walk(root);
console.log("scanned files:", scanned);
