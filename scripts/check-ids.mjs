// Audits element ids: every `$("id")` / `getElementById("id")` in the UI must
// exist in index.html (or be created by a renderer at runtime). Missing ids are
// silent no-ops at runtime, which is how several features broke before.
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const html = fs.readFileSync(path.join(dir, "index.html"), "utf8");
const files = fs.readdirSync(dir).filter((file) => file.endsWith(".js"));

// Ids present in the static document.
const present = new Set();
for (const match of html.matchAll(/\bid="([^"]+)"/g)) present.add(match[1]);

// Ids the renderers inject at runtime (Rust markup or JS templates).
const runtime = new Set();
const rust = fs
  .readdirSync("crates/wa-ui/src")
  .filter((file) => file.endsWith(".rs"))
  .map((file) => fs.readFileSync(path.join("crates/wa-ui/src", file), "utf8"))
  .join("\n");
for (const match of rust.matchAll(/\bid="([^"]+)"/g)) runtime.add(match[1]);
for (const file of files) {
  const source = fs.readFileSync(path.join(dir, file), "utf8");
  for (const match of source.matchAll(/\bid="([^"]+)"/g)) runtime.add(match[1]);
  for (const match of source.matchAll(/\bid=\\"([^"\\]+)\\"/g)) runtime.add(match[1]);
}

const missing = new Map();
for (const file of files) {
  const source = fs.readFileSync(path.join(dir, file), "utf8");
  const lines = source.split(/\r?\n/);
  lines.forEach((line, index) => {
    const code = line.replace(/\/\/.*$/, "");
    for (const match of code.matchAll(/(?:\$\(|getElementById\()\s*"([A-Za-z0-9_-]+)"/g)) {
      const id = match[1];
      if (present.has(id) || runtime.has(id)) continue;
      if (!missing.has(id)) missing.set(id, []);
      missing.get(id).push(`${file}:${index + 1}`);
    }
  });
}

for (const [id, where] of [...missing].sort()) {
  console.log(`#${id}  <-  ${where.slice(0, 3).join(", ")}`);
}
console.log(`\n${missing.size} missing element id(s), ${present.size} in index.html`);
