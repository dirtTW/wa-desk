// Audits data-* attributes: every `data-foo` the UI reads (via dataset or a
// selector) should be emitted by index.html, the Rust renderers or a JS
// template. A typo silently stops a handler from matching.
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const html = fs.readFileSync(path.join(dir, "index.html"), "utf8");
const files = fs.readdirSync(dir).filter((file) => file.endsWith(".js"));
const sources = files.map((file) => [file, fs.readFileSync(path.join(dir, file), "utf8")]);
const rust = fs
  .readdirSync("crates/wa-ui/src")
  .filter((file) => file.endsWith(".rs"))
  .map((file) => fs.readFileSync(path.join("crates/wa-ui/src", file), "utf8"))
  .join("\n");

const kebab = (name) =>
  name.replace(/[A-Z]/g, (ch) => `-${ch.toLowerCase()}`).replace(/^-/, "");

// Emitted attributes: static HTML, Rust markup and JS templates.
const emitted = new Set();
for (const source of [html, rust, ...sources.map(([, text]) => text)]) {
  for (const match of source.matchAll(/[`"'\s]data-([a-z0-9-]+)/g)) emitted.add(match[1]);
}
// dataset.fooBar reads data-foo-bar.
// Attributes the JS assigns itself (runtime flags) never need markup.
const assigned = new Set();
for (const [, text] of sources) {
  for (const match of text.matchAll(/\.dataset\.([A-Za-z0-9_]+)\s*=/g)) {
    assigned.add(kebab(match[1].replace(/_/g, "-")));
  }
  for (const match of text.matchAll(/setAttribute\(\s*"data-([a-z0-9-]+)"/g)) {
    assigned.add(match[1]);
  }
}

const read = new Map();
const addRead = (name, where) => {
  const key = kebab(name);
  if (!read.has(key)) read.set(key, []);
  read.get(key).push(where);
};
for (const [file, source] of sources) {
  const lines = source.split(/\r?\n/);
  lines.forEach((line, index) => {
    const code = line.replace(/\/\/.*$/, "");
    for (const match of code.matchAll(/\.dataset\.([A-Za-z0-9_]+)/g)) {
      addRead(match[1].replace(/_/g, "-"), `${file}:${index + 1}`);
    }
    for (const match of code.matchAll(/\[data-([a-z0-9-]+)/g)) {
      addRead(match[1], `${file}:${index + 1}`);
    }
  });
}

const missing = [...read].filter(([name]) => !emitted.has(name) && !assigned.has(name));
for (const [name, where] of missing.sort()) {
  console.log(`data-${name}  <-  ${where.slice(0, 3).join(", ")}`);
}
console.log(`\n${missing.length} unemitted data attribute(s) of ${read.size} read, ${emitted.size} emitted, ${assigned.size} written by JS`);
