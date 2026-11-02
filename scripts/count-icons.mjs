/* Count Material (960-grid) vs remaining legacy 24-grid icon svgs. */
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const files = fs
  .readdirSync(dir)
  .filter((f) => (f.endsWith(".js") && f !== "emoji-data.js") || f.endsWith(".html"));
let mat = 0;
let old24 = 0;
const samples = [];
const svgRe = /<svg\b[^>]*>[\s\S]*?<\/svg>/g;
for (const f of files) {
  const c = fs.readFileSync(path.join(dir, f), "utf8");
  let m;
  while ((m = svgRe.exec(c))) {
    if (m[0].includes("0 -960 960 960")) {
      mat += 1;
      continue;
    }
    if (m[0].includes('viewBox="0 0 24 24"')) {
      old24 += 1;
      if (samples.length < 10)
        samples.push(`${f}: ${m[0].replace(/\s+/g, " ").slice(0, 110)}`);
    }
  }
}
console.log(`material-960 svgs: ${mat} | remaining 24-grid svgs: ${old24}`);
for (const s of samples) console.log("  LEFT:", s);
