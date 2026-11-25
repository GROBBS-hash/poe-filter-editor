// Builds core/data/poe1_craft.json for the Crafting page: which mods can roll on each base (tiers, levels, weights),
// what each essence makes, how fossils change the odds, and what the crafting bench can add.
//
// Source: RePoE (https://github.com/repoe-fork/repoe-fork), data exported from the game's own files.
// Kept: bases you can craft on (armour, weapons, jewellery, quivers, shields, jewels, abyss jewels, cluster jewels),
// their prefix/suffix pool, essence and bench mods. Left out: uniques, flasks, Eldritch implicits, crucible, scourge.
// Cluster jewels: one base per enchant ("Large Cluster Jewel: Lightning Damage") with the enchant's tag added, since
// the enchant decides which notables can roll and how often.
//
// Run: node tools/build_craft_data.mjs
import { writeFileSync } from "node:fs";

const SRC = "https://repoe-fork.github.io";
const get = async (n) => (await fetch(`${SRC}/${n}.min.json`)).json();
const [mods, bases, essences, fossils, bench, clusterJewels] = await Promise.all(
  ["mods", "base_items", "essences", "fossils", "crafting_bench_options", "cluster_jewels"].map(get));

const CLASSES = new Set(["Helmet", "Body Armour", "Gloves", "Boots", "Shield", "Belt", "Ring", "Amulet", "Quiver",
  "Bow", "Claw", "Dagger", "Rune Dagger", "Wand", "Sceptre", "Staff", "Warstaff", "One Hand Sword", "Thrusting One Hand Sword",
  "Two Hand Sword", "One Hand Axe", "Two Hand Axe", "One Hand Mace", "Two Hand Mace", "Jewel", "AbyssJewel"]);
// Mod domains: a mod only rolls on a base of its own domain ("" = normal items).
const DOMAINS = { item: "", jewel: "jewel", abyss_jewel: "abyss_jewel", affliction_jewel: "affliction_jewel" };
// Jewel bases nobody crafts (Assembled Eye is a league special).
const SKIP = new Set(["Mycelium Core", "Assembled Eye Jewel"]);

// Bases: name -> [class, drop level, tags, domain]. Influence tags ("helmet_shaper") are added in the app.
const outBases = {};
const clusters = {};
for (const b of Object.values(bases)) {
  if (!(b.domain in DOMAINS) || b.release_state !== "released" || !CLASSES.has(b.item_class) || SKIP.has(b.name)) continue;
  if (outBases[b.name]) continue;
  if (b.domain === "affliction_jewel") {
    const cj = Object.values(clusterJewels).find((c) => c.name === b.name);
    for (const s of cj?.passive_skills ?? []) {
      const name = `${b.name}: ${s.name}`;
      outBases[name] = [b.item_class, b.drop_level, [...b.tags, s.tag], b.domain];
      clusters[name] = { size: b.name, enchant: s.stat_text, min: cj.min_skills, max: cj.max_skills };
    }
    continue;
  }
  outBases[b.name] = [b.item_class, b.drop_level, b.tags, DOMAINS[b.domain]];
}

// Mods: everything that can roll as a prefix/suffix on an item, plus essence and bench mods.
const keep = new Set();
for (const [id, m] of Object.entries(mods)) {
  if (m.domain in DOMAINS && (m.generation_type === "prefix" || m.generation_type === "suffix") && m.spawn_weights.some((s) => s.weight > 0)) keep.add(id);
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
  // Only on bases with one of these tags (empty = any).
  only: f.allowed_tags,
}));

const outMods = [];
for (const id of keep) {
  const m = mods[id];
  if (m.generation_type !== "prefix" && m.generation_type !== "suffix") continue;
  outMods.push({
    id, name: m.name, gen: m.generation_type === "prefix" ? "p" : "s", lvl: m.required_level, text: (m.text ?? "").replace(/\[([^\]|]*)\|([^\]]*)\]/g, "$2").replace(/\[([^\]]*)\]/g, "$1"),
    type: m.type, group: m.groups[0] ?? m.type, tags: m.implicit_tags,
    sw: m.spawn_weights.map((s) => [s.tag, s.weight]),
    // Percentage multipliers on the spawn weight, first matching tag (caster weapons and attack mods).
    ...(m.generation_weights?.length ? { gw: m.generation_weights.map((s) => [s.tag, s.weight]) } : {}),
    src: m.domain === "crafted" ? "bench" : m.is_essence_only ? "essence" : "",
    dom: DOMAINS[m.domain] ?? "",
  });
}

writeFileSync(new URL("../core/data/poe1_craft.json", import.meta.url),
  JSON.stringify({ bases: outBases, mods: outMods, essences: outEss, fossils: outFos, bench: outBench, clusters }));
console.log(`bases ${Object.keys(outBases).length} (cluster ${Object.keys(clusters).length}), mods ${outMods.length}, essences ${outEss.length}, fossils ${outFos.length}, bench ${outBench.length}`);
