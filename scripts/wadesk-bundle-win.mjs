import fs from "node:fs";
const s = fs.readFileSync(process.argv[2], "latin1");
const needles = process.argv.slice(3);
for (const n of needles) {
  let from = 0;
  let count = 0;
  while (count < 3) {
    const i = s.indexOf(n, from);
    if (i < 0) break;
    console.log(`\n======== ${n} @ ${i} ========`);
    console.log(s.slice(Math.max(0, i - 700), i + 700));
    from = i + n.length;
    count++;
  }
  if (count === 0) console.log(`\n======== ${n}: NOT FOUND ========`);
}
