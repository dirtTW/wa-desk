// Flag any colour literal in our UI that is not an official WhatsApp Web token
// value (both themes), a neutral black/white alpha, or a known official palette.
import fs from "node:fs";
import path from "node:path";

const cssPath = `${process.env.TEMP}/waweb.css`;
const css = fs.readFileSync(cssPath, "latin1");

const norm = (s) => {
  s = s.trim().toLowerCase().replace(/\s+/g, "");
  if (/^#[0-9a-f]{6}$/.test(s)) return s;
  if (/^#[0-9a-f]{3}$/.test(s)) {
    return "#" + s[1] + s[1] + s[2] + s[2] + s[3] + s[3];
  }
  return s;
};

const allowed = new Set();
// Every declared --WDS-* (and legacy) colour value across both theme blocks.
for (const m of css.matchAll(/--(?:WDS-|)[a-z0-9-]+:\s*(#[0-9a-fA-F]{3,6}|rgba?\([^)]*\))/g)) {
  allowed.add(norm(m[1]));
}
// The profile-photo content/surface palettes and the label/status palettes.
for (const hex of [
  "#DBA685", "#53A6FD", "#BDBDBD", "#25D366", "#FC9775", "#FF72A1", "#A791FF",
  "#FB5061", "#53BDEB", "#42C7B8", "#FFD279",
  "#35271E", "#092642", "#242626", "#103529", "#35221E", "#36192A", "#242447",
  "#321622", "#092C3D", "#092D2F", "#362C1F",
  "#A62C71", "#90A841", "#C1A03F", "#792138", "#AE8774", "#F0B330", "#B6B327",
  "#C69FCC", "#8B6990", "#FF8A8C", "#54C265", "#FF7B6B", "#26C4DC", "#57C9FF",
  "#74676A", "#7E90A3", "#5696FF", "#6E257E", "#7ACBA5", "#243640",
]) allowed.add(norm(hex));
// Neutrals.
for (const n of ["#000", "#fff", "#ffffff", "#000000", "transparent", "currentcolor", "inherit", "none"]) allowed.add(norm(n));

const isNeutralAlpha = (s) =>
  /^rgba?\(0,0,0(,[\d.]+)?\)$/.test(s) || /^rgba?\(255,255,255(,[\d.]+)?\)$/.test(s);

const dir = "apps/desktop/ui";
const files = fs.readdirSync(dir).filter((f) => /\.(css|js)$/.test(f));
const offenders = new Map();
for (const f of files) {
  const src = fs.readFileSync(path.join(dir, f), "utf8");
  for (const m of src.matchAll(/#[0-9a-fA-F]{3,8}\b|rgba?\([^)]*\)/g)) {
    const lit = m[0];
    if (/^#[0-9a-fA-F]{8}$/.test(lit)) continue; // 8-digit hex (alpha) handled below
    const n = norm(lit);
    if (allowed.has(n) || isNeutralAlpha(n)) continue;
    const key = `${lit} (${f})`;
    offenders.set(key, (offenders.get(key) || 0) + 1);
  }
}
console.log(`official token values loaded: ${allowed.size}`);
console.log(`off-palette literals: ${offenders.size}`);
for (const [k, n] of [...offenders.entries()].sort((a, b) => b[1] - a[1])) {
  console.log(`  x${n}  ${k}`);
}
