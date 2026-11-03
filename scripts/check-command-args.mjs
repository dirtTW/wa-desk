// Audits Tauri command arguments: top-level keys of the second `invoke()` object
// must be camelCase (Tauri renames Rust snake_case params), except when the whole
// argument is a single struct (then its inner keys follow serde).
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const files = fs.readdirSync(dir).filter((file) => file.endsWith(".js"));

// Rust command param names per command, so we can name the expected camelCase.
const rust = fs
  .readdirSync("apps/desktop/src-tauri/src/commands")
  .filter((file) => file.endsWith(".rs"))
  .map((file) => fs.readFileSync(path.join("apps/desktop/src-tauri/src/commands", file), "utf8"))
  .join("\n");
const params = new Map();
for (const match of rust.matchAll(/#\[tauri::command\]\s*pub (?:async )?fn (\w+)(?:<[^>]*>)?\(([\s\S]*?)\)\s*(?:->|\{)/g)) {
  const name = match[1];
  const names = [];
  for (const part of match[2].split(",")) {
    const trimmed = part.trim();
    if (!trimmed) continue;
    const id = trimmed.match(/^(?:mut\s+)?([a-z_][a-z0-9_]*)\s*:/);
    if (id) names.push(id[1]);
  }
  params.set(name, names);
}

const camel = (name) => name.replace(/_([a-z0-9])/g, (_, ch) => ch.toUpperCase());

const problems = [];
for (const file of files) {
  const source = fs.readFileSync(path.join(dir, file), "utf8");
  for (const match of source.matchAll(/invoke\(\s*"([a-z0-9_]+)",\s*\{/g)) {
    const command = match[1];
    // Find the matching closing brace of the argument object.
    let depth = 0;
    let start = match.index + match[0].length - 1;
    let end = start;
    for (let i = start; i < source.length; i += 1) {
      const ch = source[i];
      if (ch === "{") depth += 1;
      else if (ch === "}") {
        depth -= 1;
        if (depth === 0) {
          end = i;
          break;
        }
      }
    }
    const body = source.slice(start + 1, end);
    // Top-level keys only (depth 0).
    let level = 0;
    let line = "";
    const keys = [];
    for (const ch of body) {
      if (ch === "{" || ch === "[" || ch === "(") level += 1;
      else if (ch === "}" || ch === "]" || ch === ")") level -= 1;
      if (level === 0 && (ch === "," || ch === "\n")) {
        const key = line.match(/([A-Za-z_][\w]*)\s*:/);
        if (key) keys.push(key[1]);
        line = "";
        continue;
      }
      if (level === 0) line += ch;
    }
    const key = line.match(/([A-Za-z_][\w]*)\s*:/);
    if (key) keys.push(key[1]);

    const expected = params.get(command);
    for (const k of keys) {
      if (!k.includes("_")) continue;
      const want = camel(k);
      problems.push(
        `${file}: invoke("${command}") sends "${k}"${expected && expected.includes(k) ? ` (Rust param "${k}" -> "${want}")` : ""}`,
      );
    }
  }
}

for (const line of problems) console.log(line);
console.log(`\n${problems.length} snake_case top-level command argument(s)`);
