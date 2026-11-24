/** Which group a stat line goes in (first match wins), its title and the game item pictured. Threads before the
 *  mechanics that have threads, so "Threads found" lands in Threads; "chance to be Abyss" lands in Abyss. */
export const GROUPS: [string, string, string, RegExp][] = [
  ["shrines", "Shrines", "Shrine Map", /Shrine/],
  ["harvest", "Harvest", "Wild Crystallised Lifeforce", /Sacred Flora|Sacred Fauna|Lifeforce|Harvest|Wildwood|Crops|Sacred Grove/],
  ["breach", "Breach", "Breach Thread", /Breach|Hiveblood|Wombgift|Grasping Coffer/],
  ["abyss", "Abyss", "Abyss Thread", /Abyss/],
  ["legion", "Legion", "Legion Thread", /Legion/],
  ["ritual", "Ritual", "Ritual Thread", /Ritual/],
  ["blight", "Blight", "Blight Thread", /Blight|Oils?\b/],
  ["essence", "Essence", "Deafening Essence of Greed", /Essence/],
  ["betrayal", "Betrayal", "Betrayal Thread", /Syndicate|Betrayal/],
  ["incursion", "Incursion", "Incursion Thread", /Incursion|Alva|Temple/],
  ["expedition", "Expedition", "Expedition Thread", /Expedition|Logbook|Remnant|Artifact/],
  ["bestiary", "Bestiary", "Bestiary Thread", /Beast|Einhar/],
  ["ultimatum", "Ultimatum", "Ultimatum Thread", /Ultimatum/],
  ["delirium", "Delirium", "Simulacrum Splinter", /Delirium|Simulacrum/],
  ["bosses", "Bosses", "Fragment of the Minotaur", /Map Boss|Unique Boss|Conqueror|Maven|Eater of Worlds|Searing Exarch|Shaper|Elder/],
  ["memories", "Memories", "Alva's Memory", /Memor/],
  ["settlers", "Settlers", "Kalguuran Scarab", /Kalguur|Settler/],
  ["mercenaries", "Mercenaries", "Gold", /Mercenar/],
  ["divination cards", "Div cards", "Stacked Deck", /Divination/],
  ["strongboxes", "Strongboxes", "", /Strongbox/],
  ["rogue exiles", "Rogue exiles", "", /Rogue Exile/],
  ["torment", "Torment", "", /Torment|Possess/],
  ["mirage", "Mirage", "", /Mirage/],
  ["threads", "Threads", "Frayed Thread", /Thread/],
  ["rare monsters", "Rare monsters", "Orb of Chance", /Rare Monster/],
  ["magic monsters", "Magic monsters", "Orb of Augmentation", /Magic Monster/],
  ["maps", "Maps and loot", "Orb of Horizons", /./],
];

/** Atlas areas (the big names on the tree) for each group, where the name differs from the group title. */
export const AREAS: Record<string, string[]> = {
  harvest: ["Sacred Flora", "Viridian Wildwood"],
  bosses: ["Unique Monsters", "Conquerors", "The Shaper and Elder", "The Searing Exarch", "The Eater of Worlds"],
  memories: ["Atlas Memories"],
  settlers: ["Settlers of Kalguur"],
  "divination cards": ["Divination Cards"],
  maps: ["Maps"],
};

/** The group a search names ("harvest", "div", "shrines"…), if any: matched on its key or title. */
export function groupFor(q: string) {
  const w = q.trim().toLowerCase().replace(/s$/, "");
  if (w.length < 3) return undefined;
  return GROUPS.filter(([key]) => key !== "maps").find(([key, title]) => [key, title.toLowerCase()].some((n) => n.replace(/s$/, "").startsWith(w) || w.startsWith(n.replace(/s$/, ""))));
}
