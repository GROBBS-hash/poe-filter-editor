// Builds public/atlas_picks.json: the creator strategies shown under "Creator picks" on the Atlas page.
// A hand-kept list of maxroll farming strategies (ids below), each read once from maxroll's public
// strategy endpoint (one request a second) for its name, creator, mechanics, league and last update.
// The tree itself is read when he loads a pick, so it's always the creator's latest.
//
// Refresh each league: update the ids from maxroll.gg/poe/farming-strategy-planner/maxroll-strategies and
// /community-strategies, then: node tools/build_atlas_picks.mjs
import fs from "node:fs";

const UA = "poe-filter-editor/0.1 (local app)";
// Maxroll's own team picks, then community ones (from the lists above, 23 Nov 2026, Covens of Umbra 3.30).
// id -> mechanics (by hand: maxroll often leaves them blank). The picker filters on these.
const MAXROLL = {
  xn4dwq0g: ["legion", "abyss", "bestiary", "rare monsters"], li2pay02: ["abyss", "breach", "bestiary"],
  ue4lfx0g: ["harvest", "essence"], "3j41p602": ["blight", "harvest"], ew296k0g: ["incursion"], "9l355o0g": ["blight"],
  n1cm70gm: ["bosses"], oz1lwj0g: ["essence", "shrines"], vg1ga30g: ["shrines", "magic monsters"],
};
// Community picks with a real name and a tree (the newest list is full of unnamed tests).
const COMMUNITY = {
  o53tct02: ["betrayal"], i35nlu02: ["harvest", "settlers"], q056du0g: ["breach", "harvest", "abyss"],
  jq52i20g: ["abyss", "delirium"], "1q5lb90g": ["abyss", "breach", "legion"], jk2yun02: ["bosses", "shrines", "mercenaries"],
  af3cit0g: ["incursion", "shrines"], mp3c6z0g: ["incursion", "essence"], "4239rr02": ["memories"],
  "3r3l5a02": ["harvest", "ritual", "expedition", "betrayal"],
};

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const parse = (x) => (typeof x === "string" ? (() => { try { return JSON.parse(x); } catch { return null; } })() : x);

const picks = [];
for (const [group, ids] of [["maxroll", MAXROLL], ["community", COMMUNITY]]) {
  for (const [id, tags] of Object.entries(ids)) {
    const r = await fetch(`https://planners.maxroll.gg/profiles/poe/${id}`, { headers: { "User-Agent": UA, Accept: "application/json" } });
    if (!r.ok) { console.warn(`${id}: ${r.status}`); await sleep(1000); continue; }
    const v = await r.json();
    const meta = parse(v.metadata) ?? {};
    const data = parse(v.data) ?? {};
    // Points in the biggest tree (history = hand-picked nodes; auto-pathing adds a few, so this is a floor).
    let points = 0;
    for (const p of data.profiles ?? []) for (const s of p.atlas?.variants ?? []) points = Math.max(points, (s.history ?? []).length);
    const mechanics = [...new Set([...tags, ...(v.search_metadata?.mechanics ?? []), ...((meta.strategy?.threads ?? []).map((t) => t.mechanic))])].filter(Boolean);
    if (points < 5) { console.warn(`${id}: no tree, skipped`); await sleep(1000); continue; }
    picks.push({
      id, group,
      name: (v.name ?? "").trim(),
      author: meta.author?.contentCreator || v.user?.username || "",
      mechanics,
      category: v.category ?? "",
      season: v.season ?? "",
      updated: (v.date ?? "").slice(0, 10),
      maps: v.search_metadata?.items ?? [],
      points,
    });
    console.log(`${group} ${id} ${v.name} · ${points} pts · ${mechanics.join(", ")} · ${v.season} · ${v.date}`);
    await sleep(1000);
  }
}
fs.writeFileSync(new URL("../public/atlas_picks.json", import.meta.url),
  JSON.stringify({ built: new Date().toISOString().slice(0, 10), picks }, null, 1) + "\n");
console.log(`${picks.length} picks written`);
