// Builds core/data/poe1_dust.json: how much Thaumaturgic Dust each unique gives when disenchanted in Kingsmarch,
// grouped by the base it drops on (a loot filter can only see the base).
//   node tools/build_dust.mjs
// Sources (fetched live, once each):
//   poedb.tw/us/Kingsmarch, "Disenchant" tab: every unique's disenchant value, from the game's data. The page gives the
//     game's formula: dust = 5 * floor(25 * value * clamp(ilvl - 64, 1, 20)
//                                    * (1 + 0.5 * (influences + corrupted implicits) + 2 * quality / 100) + 0.01)
//   pathofexile.com/api/trade/data/items: which base(s) each unique is on.
import fs from "node:fs";

const UA = { "User-Agent": "poe-filter-editor (local build script)" };
const get = async (url) => { const r = await fetch(url, { headers: UA }); if (!r.ok) throw new Error(`${url}: ${r.status}`); return r.text(); };

const page = await get("https://poedb.tw/us/Kingsmarch");
const at = page.indexOf('id="Disenchant"');
if (at < 0) throw new Error("poedb has no Disenchant tab any more");
const end = page.indexOf('id="Kingsmarch"', at);
const strip = (s) => s.replace(/<[^>]+>/g, "").replace(/&#0?39;|&apos;/g, "'").replace(/&amp;/g, "&").trim();
const value = new Map();
for (const row of page.slice(at, end > 0 ? end : undefined).matchAll(/<tr>(.*?)<\/tr>/gs)) {
  const cells = [...row[1].matchAll(/<td[^>]*>(.*?)<\/td>/gs)].map((c) => strip(c[1]));
  if (cells.length >= 2 && !Number.isNaN(+cells[1])) value.set(cells[0], +cells[1]);
}
if (value.size < 1000) throw new Error(`only ${value.size} uniques on poedb's Disenchant tab`);

const items = JSON.parse(await get("https://www.pathofexile.com/api/trade/data/items"));
// Gear-like groups only: maps, contracts and sanctum relics aren't loot filter uniques here.
const GROUPS = new Set(["accessory", "armour", "flask", "jewel", "weapon", "tincture", "idol"]);
const bases = {};
let noValue = [];
for (const g of items.result.filter((g) => GROUPS.has(g.id))) {
  for (const e of g.entries.filter((e) => e.flags?.unique && e.name && e.type)) {
    const v = value.get(e.name);
    if (v === undefined) { noValue.push(e.name); continue; }
    const list = (bases[e.type] ??= []);
    if (!list.some(([n]) => n === e.name)) list.push([e.name, v]);
  }
}
for (const l of Object.values(bases)) l.sort((a, b) => a[1] - b[1] || a[0].localeCompare(b[0]));
const sorted = Object.fromEntries(Object.entries(bases).sort(([a], [b]) => a.localeCompare(b)));
const out = { updated: new Date().toISOString().slice(0, 10), source: "poedb.tw/us/Kingsmarch (Disenchant) + pathofexile.com trade items", bases: sorted };
fs.writeFileSync(new URL("../core/data/poe1_dust.json", import.meta.url), JSON.stringify(out, null, 0) + "\n");
noValue = [...new Set(noValue)];
console.log(`${value.size} dust values, ${Object.keys(sorted).length} bases; no dust value for ${noValue.length}: ${noValue.join(", ")}`);
