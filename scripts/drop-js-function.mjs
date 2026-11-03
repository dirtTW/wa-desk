// Removes top-level `function name(...) { ... }` blocks (and a comment block
// directly above them) from a JS file. One-off maintenance helper.
import fs from "node:fs";

const [file, ...names] = process.argv.slice(2);
let source = fs.readFileSync(file, "utf8");
const lines = source.split(/\r?\n/);

for (const name of names) {
  const start = lines.findIndex((line) => line.startsWith(`function ${name}(`));
  if (start < 0) {
    console.error(`not found: ${name}`);
    continue;
  }
  let depth = 0;
  let started = false;
  let end = start;
  for (let i = start; i < lines.length; i += 1) {
    for (const ch of lines[i]) {
      if (ch === "{") {
        depth += 1;
        started = true;
      } else if (ch === "}") {
        depth -= 1;
      }
    }
    if (started && depth === 0) {
      end = i;
      break;
    }
  }
  // Take the doc-comment lines just above with it.
  let top = start;
  while (top > 0 && /^\s*(\/\/|\/\*|\*)/.test(lines[top - 1])) top -= 1;
  lines.splice(top, end - top + 1);
  console.log(`removed ${name} (${end - start + 1} lines)`);
}

source = lines.join("\n").replace(/\n{3,}/g, "\n\n");
fs.writeFileSync(file, source);
