// Builds core/data/poe1_craft.json for the Crafting page: which mods can roll on each base (tiers, levels, weights),
// what each essence makes, how fossils change the odds, and what the crafting bench can add.
//
// Source: RePoE (https://github.com/repoe-fork/repoe-fork), data exported from the game's own files.
// Kept: bases you can craft on (armour, weapons, jewellery, quivers, shields), their prefix/suffix pool, essence and
// bench mods. Left out: uniques, jewels, flasks, clusters, Eldritch implicits, crucible, scourge.
//
// Run: node tools/build_craft_data.mjs
import { writeFileSync } from "node:fs";

const SRC = "https://repoe-fork.github.io";
const get = async (n) => (await fetch(`${SRC}/${n}.min.json`)).json();
const [mods, bases, essences, fossils, bench] = await Promise.all(["mods", "base_items", "essences", "fossils", "crafting_bench_options"].map(get));

const CLASSES = new Set(["Helmet", "Body Armour", "Gloves", "Boots", "Shield", "Belt", "Ring", "Amulet", "Quiver",
  "Bow", "Claw", "Dagger", "Rune Dagger", "Wand", "Sceptre", "Staff", "Warstaff", "One Hand Sword", "Thrusting One Hand Sword",
  "Two Hand Sword", "One Hand Axe", "Two Hand Axe", "One Hand Mace", "Two Hand Mace"]);

// Bases: name -> [class, drop level, tags]. Influence tags ("helmet_shaper") are added in the app.
const outBases = {};
for (const b of Object.values(bases)) {
  if (b.domain !== "item" || b.release_state !== "released" || !CLASSES.has(b.item_class)) continue;
  if (outBases[b.name]) continue;
  outBases[b.name] = [b.item_class, b.drop_level, b.tags];
}

// Mods: everything that can roll as a prefix/suffix on an item, plus essence and bench mods.
const keep = new Set();
for (const [id, m] of Object.entries(mods)) {
  if (m.domain === "item" && (m.generation_type === "prefix" || m.generation_type === "suffix") && m.spawn_weights.some((s) => s.weight > 0)) keep.add(id);
}
const outEss = [];
for (const e of Object.values(essences)) {
  const ms = Object.fromEntries(Object.entries(e.mods).filter(([c, id]) => CLASSES.has(c) && mods[id]));
  if (!Object.keys(ms).length) continue;
  Object.values(ms).forEach((id) => keep.add(id));
  outEss.push({ name: e.name, level: e.level, corrupt: !!e.type?.is_corruption_only, mods: ms });
}
const outBench = [];
for (const b of bench) {
  const id = b.actions.add_explicit_mod;
  if (!id || !mods[id]) continue;
  const classes = b.item_classes.filter((c) => CLASSES.has(c));
  if (!classes.length) continue;
  keep.add(id);
  const cost = Object.entries(b.cost).map(([k, n]) => [k.split("/").pop(), n]);
  outBench.push({ mod: id, classes, cost });
}
const outFos = Object.values(fossils).filter((f) => f.name.endsWith("Fossil")).map((f) => ({
  name: f.name,
  more: f.positive_mod_weights.map((w) => [w.tag, w.weight]),
  less: f.negative_mod_weights.map((w) => [w.tag, w.weight]),
  says: [...Object.values(f.descriptions), ...Object.values(f.blocked_descriptions ?? {})],
}));

const outMods = [];
for (const id of keep) {
  const m = mods[id];
  if (m.generation_type !== "prefix" && m.generation_type !== "suffix") continue;
  outMods.push({
    id, name: m.name, gen: m.generation_type === "prefix" ? "p" : "s", lvl: m.required_level, text: (m.text ?? "").replace(/\[([^\]|]*)\|([^\]]*)\]/g, "$2").replace(/\[([^\]]*)\]/g, "$1"),
    type: m.type, group: m.groups[0] ?? m.type, tags: m.implicit_tags,
    sw: m.spawn_weights.map((s) => [s.tag, s.weight]),
    src: m.domain === "crafted" ? "bench" : m.is_essence_only ? "essence" : "",
  });
}

writeFileSync(new URL("../core/data/poe1_craft.json", import.meta.url),
  JSON.stringify({ bases: outBases, mods: outMods, essences: outEss, fossils: outFos, bench: outBench }));
console.log(`bases ${Object.keys(outBases).length}, mods ${outMods.length}, essences ${outEss.length}, fossils ${outFos.length}, bench ${outBench.length}`);
