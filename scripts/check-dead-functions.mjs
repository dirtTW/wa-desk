// Lists top-level UI functions with no remaining caller (dead code left over
// after a renderer moved to wa-ui). A name counts as used when it appears
// anywhere other than its own definition — a call, a `window.x` export, an
// `on*=` attribute in index.html, or a string reference.
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const files = fs.readdirSync(dir).filter((file) => file.endsWith(".js"));
const html = fs.readFileSync(path.join(dir, "index.html"), "utf8");
const sources = files.map((file) => [file, fs.readFileSync(path.join(dir, file), "utf8")]);
const everything = sources.map(([, text]) => text).join("\n") + "\n" + html;

const dead = [];
for (const [file, source] of sources) {
  const lines = source.split(/\r?\n/);
  lines.forEach((line, index) => {
    const match = line.match(/^(?:async\s+)?function\s+([A-Za-z_$][\w$]*)\s*\(/);
    if (!match) return;
    const name = match[1];
    // Count references outside this definition line.
    const refs = [...everything.matchAll(new RegExp(`(?<![\\w$.])${name}(?![\\w$])`, "g"))].length;
    if (refs <= 1) dead.push(`${name}  (${file}:${index + 1})`);
  });
}

for (const line of dead.sort()) console.log(line);
console.log(`\n${dead.length} function(s) with no caller`);
