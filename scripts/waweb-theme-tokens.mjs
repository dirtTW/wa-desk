// Extract the theme variant blocks (dark + light) from the WhatsApp Web CSS.
import fs from "node:fs";

const css = fs.readFileSync(process.argv[2], "latin1");
const want = [
  "--WDS-surface-default",
  "--WDS-surface-emphasized",
  "--WDS-surface-elevated-default",
  "--WDS-surface-elevated-emphasized",
  "--WDS-app-wash",
  "--WDS-content-default",
  "--WDS-content-deemphasized",
  "--WDS-content-disabled",
  "--WDS-accent",
  "--WDS-accent-deemphasized",
  "--WDS-accent-emphasized",
  "--WDS-lines-divider",
  "--WDS-lines-outline-default",
  "--WDS-components-filter-surface-selected",
  "--WDS-components-active-list-row",
  "--WDS-systems-bubble-surface-incoming",
  "--WDS-systems-bubble-surface-outgoing",
  "--WDS-systems-bubble-surface-system",
  "--WDS-systems-chat-surface-composer",
  "--WDS-systems-chat-background-wallpaper",
  "--WDS-content-external-link",
  "--WDS-content-read",
  "--WDS-secondary-negative",
  "--WDS-content-inverse",
];

const ruleRe = /([^{}]+)\{([^{}]*)\}/g;
const blocks = [];
let m;
while ((m = ruleRe.exec(css))) {
  const body = m[2];
  if (!body.includes("--WDS-surface-default:")) continue;
  const vals = {};
  for (const w of want) {
    const r = new RegExp(w.replace(/[-]/g, "\\-") + ":\\s*([^;]+)");
    const mm = body.match(r);
    if (mm) vals[w] = mm[1].trim();
  }
  blocks.push({ sel: m[1].trim().slice(0, 60), vals });
}
console.log("theme blocks defining --WDS-surface-default:", blocks.length);

function report(title, pred) {
  const b = blocks.find((x) => pred(x.vals));
  if (!b) {
    console.log(`\n### ${title}: NOT FOUND`);
    return;
  }
  console.log(`\n### ${title}  (selector ${b.sel})`);
  for (const [k, v] of Object.entries(b.vals)) console.log(`  ${k} = ${v}`);
}

report("DARK (surface #161717)", (v) => /#161717/i.test(v["--WDS-surface-default"] || ""));
report("LIGHT (surface #FFFFFF)", (v) => /#ffffff/i.test(v["--WDS-surface-default"] || ""));
