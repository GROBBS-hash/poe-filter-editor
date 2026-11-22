// Builds core/data/levelling.json (the levelling overlay's route, zones and quest rewards) from
// HeartofPhos/exile-leveling (MIT, https://github.com/HeartofPhos/exile-leveling).
//
//   git clone --depth 1 https://github.com/HeartofPhos/exile-leveling.git <somewhere>
//   node tools/build_levelling.mjs <somewhere>/exile-leveling
//
// Route lines look like:  "➞ {enter|1_1_2} #The Coast"  with "#sub" hint lines under them and
// "#ifdef LEAGUE_START" ... "#endif" blocks. We keep the tags as parts so the overlay can colour them.
import fs from "node:fs";
import path from "node:path";

const src = process.argv[2];
if (!src) { console.error("usage: node tools/build_levelling.mjs <exile-leveling folder>"); process.exit(1); }
const data = path.join(src, "common", "data");
const read = (p) => JSON.parse(fs.readFileSync(path.join(data, "json", p), "utf8"));

const areasRaw = read("areas.json");
const gemsRaw = read("gems.json");
const questsRaw = read("quests.json");

const areas = {};
for (const a of Object.values(areasRaw)) {
  areas[a.id] = { n: a.name, a: a.act, l: a.level, t: !!a.is_town_area };
}
const gemName = (id) => gemsRaw[id]?.name;

const quests = {};
for (const q of Object.values(questsRaw)) {
  const rewards = [];
  const vendor = [];
  let npc = "";
  for (const offer of Object.values(q.reward_offers)) {
    npc = npc || offer.quest_npc || "";
    for (const [id, v] of Object.entries(offer.quest ?? {})) {
      const n = gemName(id);
      if (n) rewards.push([n, v.classes ?? []]);
    }
    for (const [id, v] of Object.entries(offer.vendor ?? {})) {
      const n = gemName(id);
      if (n) vendor.push([n, v.classes ?? [], v.npc ?? ""]);
    }
  }
  quests[q.id] = { name: q.name, act: Number(q.act), npc, rewards, vendor };
}

// Every gem name (for short vendor search strings that match only your gems).
const gems = [...new Set(Object.values(gemsRaw).map((g) => g.name))].sort();

const steps = [];
let act = 0;
const conds = [];
for (let n = 1; n <= 10; n++) {
  const lines = fs.readFileSync(path.join(data, "routes", `act-${n}.txt`), "utf8").split(/\r?\n/);
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith("#section")) { act = n; continue; }
    if (line.startsWith("#ifdef ")) { conds.push(line.slice(7).trim()); continue; }
    if (line.startsWith("#ifndef ")) { conds.push("!" + line.slice(8).trim()); continue; }
    if (line.startsWith("#endif")) { conds.pop(); continue; }
    if (line.startsWith("#sub ")) {
      const last = steps[steps.length - 1];
      if (last) last.subs.push(line.slice(5).trim());
      continue;
    }
    // "text {tag|value} text #comment": the comment is only a reminder of the tag's name.
    const body = line.replace(/\s#[^{}]*$/, "").trim();
    const parts = [];
    let i = 0;
    const re = /\{([a-z_]+)(?:\|([^}]*))?\}/g;
    let m;
    while ((m = re.exec(body))) {
      if (m.index > i) parts.push(["text", body.slice(i, m.index)]);
      parts.push([m[1], m[2] ?? ""]);
      i = m.index + m[0].length;
    }
    if (i < body.length) parts.push(["text", body.slice(i)]);
    const step = { act: act || n, parts, subs: [] };
    if (conds.length) step.cond = [...conds];
    steps.push(step);
  }
}

const out = {
  credit: "Route, zones and quest rewards from exile-leveling by HeartofPhos (MIT).",
  areas, quests, gems, steps,
};
const dest = path.join(path.dirname(new URL(import.meta.url).pathname), "..", "core", "data", "levelling.json");
fs.writeFileSync(dest, JSON.stringify(out));
console.log(`steps ${steps.length}, areas ${Object.keys(areas).length}, quests ${Object.keys(quests).length}, gems ${gems.length} -> ${dest}`);
