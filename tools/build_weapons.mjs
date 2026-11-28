// Builds core/data/poe1_weapons.json: every released PoE 1 weapon base with its damage, for Upgrades on weapons
// (28 Nov 2026). From RePoE's export of the game data (repoe-fork.github.io/base_items.json, ~8 MB).
//   node tools/build_weapons.mjs [path/to/base_items.json]   (downloads it when no path is given)
// Each base: [physical min, physical max, attack time in ms, crit chance in hundredths of a %, item class, level].
import fs from "node:fs";

const src = process.argv[2];
const all = src ? JSON.parse(fs.readFileSync(src, "utf8"))
  : await (await fetch("https://repoe-fork.github.io/base_items.json", { headers: { "User-Agent": "poe-filter-editor/0.1 (local app)" } })).json();
const out = {};
for (const b of Object.values(all)) {
  const p = b.properties ?? {};
  if (b.release_state !== "released" || p.physical_damage_min == null || !p.attack_time) continue;
  out[b.name] = [p.physical_damage_min, p.physical_damage_max, p.attack_time, p.critical_strike_chance ?? 0, b.item_class, b.requirements?.level ?? 1];
}
const sorted = Object.fromEntries(Object.entries(out).sort(([a], [b]) => a.localeCompare(b)));
fs.writeFileSync(new URL("../core/data/poe1_weapons.json", import.meta.url), JSON.stringify(sorted) + "\n");
console.log(`${Object.keys(sorted).length} weapon bases`);
