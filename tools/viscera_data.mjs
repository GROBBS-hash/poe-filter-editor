// Wickerman body parts (3.30 "Viscera"): name, body slot, who drops them, and the Low / Med / High versions' mods.
// From poedb's list (the trade site only has the names). Re-run after a patch that adds parts:
//   node tools/viscera_data.mjs            -> core/data/poe1_viscera.json
import { writeFileSync } from "node:fs";

const html = await (await fetch("https://poedb.tw/us/Viscera", { headers: { "User-Agent": "Mozilla/5.0" } })).text();
const text = (s) => s.replace(/<[^>]+>/g, "").replace(/&#0?39;|&apos;/g, "'").replace(/&amp;/g, "&").replace(/\s+/g, " ").trim()
  .replace(/\((\d+(?:\.\d+)?)—(\d+(?:\.\d+)?)\)/g, "($1-$2)");
const parts = [];
// One card per part: picture, name, then its properties and the three mods.
for (const card of html.split('<div class="d-flex border-top rounded">').slice(1)) {
  const img = card.match(/<img[^>]*src="([^"]+Viscera[^"]+)"/);
  const name = card.match(/class="whiteitem Viscera"[^>]*>([^<]+)<\/a><div>/);
  if (!img || !name) continue;
  const props = [...card.matchAll(/<div class="property">([\s\S]*?)<\/div>/g)].map((m) => text(m[1]));
  const slot = props[0];
  const drops = props.filter((p) => p.startsWith("Drop:")).map((p) => p.slice(5).trim());
  // Each version: its label (Low / Med / High, or poedb's "stats" / "Uber" for one-off parts), its mod lines, and the
  // trade search poedb links to (league left out: the app adds yours).
  const tiers = [...card.matchAll(/<div class="explicitMod"><a [^>]*href="([^"]+)"[^>]*>([^<]+)<\/a>\s*<span class="">([\s\S]*?)<\/span><\/div>/g)]
    .map((m) => ({
      tier: m[2].replace(/^stats\d*$/, ""),
      mods: m[3].split(/<br\s*\/?>/).map(text).filter(Boolean),
      query: JSON.parse(decodeURIComponent((m[1].replace(/&amp;/g, "&").split("?q=")[1] ?? "null").replace(/\+/g, " "))).query,
    }));
  const n = text(name[1]);
  if (parts.some((p) => p.name === n)) continue;
  parts.push({
    name: n, slot, drops, tiers,
    // Our item pictures come from GGG's CDN; poedb's path matches it (webp -> png).
    icon: img[1].replace(/^https:\/\/cdn\.poedb\.tw\/image\//, "https://web.poecdn.com/image/").replace(/\.webp$/, ".png"),
  });
}
if (parts.length < 100) throw new Error(`only ${parts.length} parts found; poedb's page changed?`);
writeFileSync(new URL("../core/data/poe1_viscera.json", import.meta.url), JSON.stringify(parts, null, 0) + "\n");
console.log(`${parts.length} parts`, parts.find((p) => p.name === "Bandit Rogue's Head"));
