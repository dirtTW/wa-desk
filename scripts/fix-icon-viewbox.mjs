/* Material Symbols artwork lives on the -960..960 grid, so every replaced
   icon svg needs viewBox="0 -960 960 960" instead of "0 0 24 24".
   Detection: the first path starts with a coordinate pair like M160-200 /
   m640-480 (number immediately followed by a minus) — old 24-grid icons are
   all gone by the time this runs. Usage: node scripts/fix-icon-viewbox.mjs */
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const files = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith(".js") || f.endsWith(".html"))
  .filter((f) => f !== "emoji-data.js");

const symRe = /(<svg\b[^>]*?)viewBox="0 0 24 24"([^>]*>)(\s*)<path\b[^>]*\bd="([Mm]?\d{1,3}(?:\.\d+)?-\d{1,3}(?:\.\d+)?)/g;

let fixed = 0;
for (const file of files) {
  const p = path.join(dir, file);
  const text = fs.readFileSync(p, "utf8");
  const next = text.replace(symRe, (m, pre, post, ws, d) => {
    fixed += 1;
    return `${pre}viewBox="0 -960 960 960"${post}${ws}<path d="${d}`;
  });
  if (next !== text) fs.writeFileSync(p, next);
}
console.log(`fixed viewBox on ${fixed} material svgs`);
