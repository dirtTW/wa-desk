// Lists `state.*` fields that are declared in the store literal but never read
// or written anywhere else — leftovers from renderers that moved to wa-ui.
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const files = fs.readdirSync(dir).filter((file) => file.endsWith(".js"));
const core = fs.readFileSync(path.join(dir, "core.js"), "utf8");

// The store literal.
const start = core.indexOf("const state = {");
if (start < 0) throw new Error("state literal not found");
let depth = 0;
let started = false;
let end = start;
for (let i = start; i < core.length; i += 1) {
  const ch = core[i];
  if (ch === "{") {
    depth += 1;
    started = true;
  } else if (ch === "}") {
    depth -= 1;
    if (started && depth === 0) {
      end = i;
      break;
    }
  }
}
const literal = core.slice(start, end);
const keys = [];
for (const match of literal.matchAll(/^\s{2}([A-Za-z_$][\w$]*)\s*:/gm)) keys.push(match[1]);

const everything = files
  .map((file) => fs.readFileSync(path.join(dir, file), "utf8"))
  .join("\n");

const unused = [];
for (const key of keys) {
  const refs = [...everything.matchAll(new RegExp(`state\\.${key}(?![\\w$])`, "g"))].length;
  if (refs === 0) unused.push(key);
}

for (const key of unused.sort()) console.log(`state.${key}`);
console.log(`\n${unused.length} unreferenced state field(s) of ${keys.length} declared`);
