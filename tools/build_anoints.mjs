// Builds src/data/anoints.json (the blight oil lookup) from Exile UI's anoint data
// (Lailloken/Exile-UI, MIT, data/english/anoints.json): amulet anoints = 3 oils -> a notable passive.
//
//   node tools/build_anoints.mjs <path to anoints.json>
import fs from "node:fs";
import path from "node:path";

const src = process.argv[2];
if (!src) { console.error("usage: node tools/build_anoints.mjs <anoints.json>"); process.exit(1); }
const raw = JSON.parse(fs.readFileSync(src, "utf8"));
// Exile UI numbers the oils 1-14 in this order (modules/anoints.ahk).
const OILS = ["Clear", "Sepia", "Amber", "Verdant", "Teal", "Azure", "Indigo", "Violet", "Crimson", "Black", "Opalescent", "Silver", "Golden", "Prismatic"];
const amulets = Object.entries(raw.amulets).map(([name, v]) => ({
  name,
  oils: v.recipe.split(",").map((n) => OILS[Number(n.trim()) - 1]).filter(Boolean),
  stats: v.stats.map((s) => s.replace(/\n/g, " ")),
})).sort((a, b) => a.name.localeCompare(b.name));
const out = { credit: "Anoint recipes from Exile UI by Lailloken (MIT).", updated: raw._timestamp ?? "", oils: OILS, amulets };
const dest = path.join(path.dirname(new URL(import.meta.url).pathname), "..", "src", "data", "anoints.json");
fs.writeFileSync(dest, JSON.stringify(out));
console.log(`${amulets.length} amulet anoints -> ${dest}`);
