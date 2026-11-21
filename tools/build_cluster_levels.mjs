// Cluster jewel notables -> the item level they need to roll, for "Buy base" (core/data/cluster_notables.json).
// Source: the game's mod data as exported by RePoE (https://repoe-fork.github.io/mods.min.json), domain
// "affliction_jewel", mods whose stat is local_affliction_notable_<name>; required_level is the item level.
// Checked 21 Nov 2026 against poedb.tw/us/Large_Cluster_Jewel (Inspired Oppression 75, Doryani's Lesson 68,
// Overshock 50). Re-run after a big patch:
//   node tools/build_cluster_levels.mjs path/to/mods.min.json
import fs from "node:fs";

const mods = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const out = {};
for (const m of Object.values(mods)) {
  if (m.domain !== "affliction_jewel") continue;
  for (const s of m.stats ?? []) {
    const k = s.id.match(/^local_affliction_notable_(.+)$/)?.[1];
    if (!k) continue;
    // Key: the notable's name in lower case, letters and digits only ("Doryani's Lesson" -> "doryanislesson").
    const key = k.replace(/[^a-z0-9]/g, "");
    out[key] = Math.max(out[key] ?? 0, m.required_level ?? 1);
  }
}
const sorted = Object.fromEntries(Object.entries(out).sort(([a], [b]) => a.localeCompare(b)));
fs.writeFileSync(new URL("../core/data/cluster_notables.json", import.meta.url), JSON.stringify(sorted, null, 1) + "\n");
console.log(Object.keys(sorted).length, "notables");
