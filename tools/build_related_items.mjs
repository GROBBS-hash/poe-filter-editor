// Builds core/data/related_items.json: for fragments, invitations, boss keys etc., the items that go with them
// (the rest of a fragment set) and what the boss drops. Shown in the Ctrl+D price window.
//
// Source: Awakened PoE Trade's item-drop.json (https://github.com/SnosMe/awakened-poe-trade,
// renderer/public/data/item-drop.json), MIT licence, Copyright (c) 2020 Alexander Drozdov.
// We keep names only (namespace and " // variant" stripped), mark which groups are a set you need all of,
// and add a few "made from N parts" recipes by hand.
//
// Run: node tools/build_related_items.mjs
import { writeFileSync } from "node:fs";

const SRC = "https://raw.githubusercontent.com/SnosMe/awakened-poe-trade/master/renderer/public/data/item-drop.json";
const raw = await (await fetch(SRC)).json();

const strip = (id) => id.split("::")[1].split(" // ")[0];
const uniq = (l) => [...new Set(l.map(strip))];

// Groups where every piece is needed together (a boss's fragment set, or pieces that combine into one item).
const SETS = [
  "Sacrifice at Dawn", "Mortal Ignorance", "Fragment of the Minotaur", "Fragment of Eradication", "Fragment of Knowledge",
  "Al-Hezmin's Crest", "First Piece of Brutality", "First Piece of Directions", "First Piece of Focus", "First Piece of Storms",
  "First Piece of the Arcane", "First Piece of Time",
];

const groups = raw.map((e) => {
  const group = uniq(e.query);
  return { group, set: group.some((n) => SETS.includes(n)), drops: uniq(e.items).filter((n) => !group.includes(n)) };
});

// Stackable parts that combine into a whole item.
const recipes = [
  { item: "The Maven's Writ", part: "Crescent Splinter", count: 10 },
  { item: "Simulacrum", part: "Simulacrum Splinter", count: 300 },
  ...["Eternal Empire", "Karui", "Maraketh", "Templar", "Vaal"].map((f) => ({ item: `Timeless ${f} Emblem`, part: `Timeless ${f} Splinter`, count: 100 })),
];

const out = { source: "Awakened PoE Trade item-drop.json (MIT, (c) 2020 Alexander Drozdov)", groups, recipes };
writeFileSync(new URL("../core/data/related_items.json", import.meta.url), JSON.stringify(out, null, 1) + "\n");
console.log(`${groups.length} groups, ${recipes.length} recipes`);
