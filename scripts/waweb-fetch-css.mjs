// Download WhatsApp Web's stylesheet and extract the color design tokens.
import fs from "node:fs";

const url =
  "https://static.whatsapp.net/rsrc.php/v5/y7/l/0,cross/xi9cxpVmmibGRLM5ENHN5kOB_OY8k1KqH.css";
const out = `${process.env.TEMP}/waweb.css`;

const r = await fetch(url, {
  headers: {
    "User-Agent":
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    Accept: "text/css,*/*;q=0.1",
  },
});
console.log("status", r.status, r.headers.get("content-type"));
const css = await r.text();
fs.writeFileSync(out, css);
console.log(`saved ${css.length} bytes -> ${out}`);

// Collect every `--token: <color>` declaration with the selector that declares it.
const decls = [];
const ruleRe = /([^{}]+)\{([^{}]*)\}/g;
let m;
while ((m = ruleRe.exec(css))) {
  const sel = m[1].trim().replace(/\s+/g, " ");
  const body = m[2];
  for (const d of body.split(";")) {
    const i = d.indexOf(":");
    if (i < 0) continue;
    const name = d.slice(0, i).trim();
    const val = d.slice(i + 1).trim();
    if (!name.startsWith("--")) continue;
    if (!/^#|^rgb|^hsl|^oklch|^color\(|^var\(/.test(val)) continue;
    decls.push({ sel, name, val });
  }
}
console.log("color-valued custom-property declarations:", decls.length);

// Group by selector root to see the theme blocks.
const bySel = new Map();
for (const d of decls) bySel.set(d.sel, (bySel.get(d.sel) || 0) + 1);
console.log("selectors declaring color tokens:");
for (const [sel, n] of [...bySel.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12)) {
  console.log(`  ${n.toString().padStart(4)}  ${sel.slice(0, 110)}`);
}
fs.writeFileSync(`${process.env.TEMP}/waweb-tokens.json`, JSON.stringify(decls, null, 0));
console.log("tokens ->", `${process.env.TEMP}/waweb-tokens.json`);
