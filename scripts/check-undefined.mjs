// Reports every function the UI calls but never defines (a ReferenceError at
// runtime). Scans for `name(` call sites and checks for a definition.
import fs from "node:fs";
import path from "node:path";

const dir = "apps/desktop/ui";
const files = fs.readdirSync(dir).filter((file) => file.endsWith(".js"));
const sources = new Map(files.map((file) => [file, fs.readFileSync(path.join(dir, file), "utf8")]));
const all = [...sources.values()].join("\n");

// Definitions: `function name(` / `name = (` / `const name =` / `var name =`.
const defined = new Set();
for (const match of all.matchAll(/(?:function\s+([A-Za-z_$][\w$]*)|(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=)/g)) {
  defined.add(match[1] || match[2]);
}
// Globals provided by the platform.
const builtins = new Set(
  "invoke listen emit window document navigator console Math JSON Date Object Array String Number Boolean Promise Map Set WeakMap URL Blob File FileReader Image Audio HTMLAudioElement HTMLVideoElement IntersectionObserver MutationObserver ResizeObserver requestAnimationFrame cancelAnimationFrame setTimeout clearTimeout setInterval clearInterval parseInt parseFloat isNaN encodeURIComponent decodeURIComponent atob btoa Uint8Array Int16Array Float32Array ArrayBuffer AudioContext AudioWorkletNode MediaStream MediaStreamTrackProcessor VideoEncoder VideoDecoder EncodedVideoChunk VideoFrame CSS CustomEvent Event MouseEvent KeyboardEvent NodeFilter Highlight Range TreeWalker structuredClone fetch confirm alert prompt localStorage sessionStorage performance queueMicrotask Intl RegExp Error TypeError Symbol requestIdleCallback".split(
    " ",
  ),
);

const missing = new Map();
for (const [file, source] of sources) {
  const lines = source.split(/\r?\n/);
  lines.forEach((line, index) => {
    // Skip comments and property access / method definitions.
    const code = line.replace(/\/\/.*$/, "");
    for (const match of code.matchAll(/(?<![.\w$])([a-z_$][\w$]*)\s*\(/g)) {
      const name = match[1];
      if (defined.has(name) || builtins.has(name)) continue;
      if (["if", "for", "while", "switch", "catch", "return", "typeof", "new", "await", "function", "else", "do", "case", "in", "of", "delete", "void", "yield", "throw", "super", "this"].includes(name)) continue;
      const key = `${name}`;
      if (!missing.has(key)) missing.set(key, []);
      missing.get(key).push(`${file}:${index + 1}`);
    }
  });
}

for (const [name, where] of [...missing].sort()) {
  console.log(`${name}  <-  ${where.slice(0, 4).join(", ")}`);
}
console.log(`\n${missing.size} undefined call target(s)`);
