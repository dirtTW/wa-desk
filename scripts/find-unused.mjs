/* Find top-level declarations with zero references anywhere in the UI.
   Classic scripts share the global scope, so a name declared in one file may
   be used in any other — references are counted across every file (html too).
   Only *cross-checks text references*; vet results before deleting anything.
   Usage: node scripts/find-unused.mjs */
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const files = fs
  .readdirSync(dir)
  .filter((f) => (f.endsWith(".js") && f !== "emoji-data.js") || f.endsWith(".html"));
const sources = {};
for (const f of files) sources[f] = fs.readFileSync(path.join(dir, f), "utf8");
const all = Object.values(sources).join("\n");

const names = new Set();
for (const c of Object.values(sources)) {
  for (const m of c.matchAll(/^function\s+([A-Za-z_$][\w$]*)/gm)) names.add(m[1]);
  for (const m of c.matchAll(/^const\s+([A-Za-z_$][\w$]*)\s*=/gm)) names.add(m[1]);
  for (const m of c.matchAll(/^let\s+([A-Za-z_$][\w$]*)/gm)) names.add(m[1]);
}

const dead = [];
for (const name of names) {
  const re = new RegExp(`\\b${name}\\b`, "g");
  let refs = 0;
  for (const c of Object.values(sources)) {
    for (const m of c.matchAll(re)) {
      // skip the declaration itself
      const head = c.slice(Math.max(0, m.index - 12), m.index);
      if (/\b(function|const|let)\s+$/.test(head)) continue;
      refs++;
    }
  }
  if (refs === 0) dead.push(name);
}

console.log(`declared: ${names.size} | zero-reference names: ${dead.length}`);
console.log(dead.join("\n"));
