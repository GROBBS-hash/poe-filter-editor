// Data for the build checker (core/src/buildcheck.rs) -> core/data/poe1_buildcheck.json.
// Sources (fetched live, no login):
//   Path of Building Community's game data (github.com/PathOfBuildingCommunity/PathOfBuilding, src/Data):
//     Gems.lua (attribute split), Skills/*.lua (level requirement, mana cost / multiplier, reservation per level,
//     a few stats the speed check needs), Bases/*.lua (item level / attribute requirements, weapon attack speed).
//   The official passive tree export (github.com/grindinggear/skilltree-export, data.json): node positions and the
//     stat lines the checker reads (attributes, duration, attack speed, mana, reservation).
// Checked 27 Nov 2026 against PoB 2.69 and the 3.30 tree. Re-run after a big patch:
//   node tools/build_check_data.mjs
import fs from "node:fs";

const RAW = "https://raw.githubusercontent.com/PathOfBuildingCommunity/PathOfBuilding/master/src/Data/";
const get = async (u) => { const r = await fetch(u); if (!r.ok) throw new Error(`${u}: ${r.status}`); return r.text(); };
const list = async (dir) => (await (await fetch(`https://api.github.com/repos/PathOfBuildingCommunity/PathOfBuilding/contents/src/Data/${dir}`)).json())
  .filter((f) => f.name.endsWith(".lua")).map((f) => f.name);

// ---- gems
const gemsLua = await get(RAW + "Gems.lua");
const attrSplit = {};
for (const b of gemsLua.split(/\n\t\["Metadata/).slice(1)) {
  const name = b.match(/\n\t\tname = "([^"]+)"/)?.[1];
  if (!name || attrSplit[name]) continue;
  const tags = b.match(/tags = \{([^}]*)\}/)?.[1] ?? "";
  attrSplit[name] = { s: +(b.match(/reqStr = (\d+)/)?.[1] ?? 0), d: +(b.match(/reqDex = (\d+)/)?.[1] ?? 0), i: +(b.match(/reqInt = (\d+)/)?.[1] ?? 0), sup: /support = true/.test(tags) };
}
const skillFiles = (await list("Skills")).filter((f) => /^(act|sup)_/.test(f));
const gems = {};
for (const f of skillFiles) {
  const t = await get(RAW + "Skills/" + f);
  for (const b of t.split(/\nskills\["/).slice(1)) {
    const name = b.match(/\n\tname = "([^"]+)"/)?.[1];
    // Some gems have two blocks (Spellslinger: the support part and the reserving part): merge what each adds.
    if (!name || !attrSplit[name]) continue;
    const levels = b.match(/\n\tlevels = \{([\s\S]*?)\n\t\},?\n/)?.[1] ?? "";
    const statNames = (b.match(/\n\tstats = \{([\s\S]*?)\n\t\},/)?.[1] ?? "").match(/"[^"]+"/g)?.map((s) => s.slice(1, -1)) ?? [];
    const g = { ...attrSplit[name], req: [] };
    const statAt = (stat) => statNames.indexOf(stat);
    for (const m of levels.matchAll(/\[(\d+)\] = \{([^\n]*)\},/g)) {
      const lv = +m[1], row = m[2];
      // Spectre limits to level 40 (gear lifts the gem past 21: Raise Spectre raises 3 from level 25).
      const sp = statAt("base_number_of_spectres_allowed");
      if (sp >= 0 && lv <= 40) { const v = parseFloat(row.split(",")[sp]); if (!isNaN(v)) (g.spectres ??= [])[lv - 1] = v; }
      if (lv > 21) continue;
      g.req[lv - 1] = +(row.match(/levelRequirement = (\d+)/)?.[1] ?? 1);
      const nums = row.split(/,\s*[a-zA-Z]/)[0].split(",").map((x) => parseFloat(x)).filter((x) => !isNaN(x));
      const put = (k, v) => { (g[k] ??= [])[lv - 1] = v; };
      const flat = row.match(/manaReservationFlat = (\d+)/)?.[1]; if (flat) put("resFlat", +flat);
      const pct = row.match(/manaReservationPercent = (\d+)/)?.[1]; if (pct) put("resPct", +pct);
      const cost = row.match(/cost = \{ Mana = (\d+)/)?.[1]; if (cost) put("cost", +cost);
      const mm = row.match(/manaMultiplier = (-?\d+)/)?.[1]; if (mm) put("mm", +mm);
      const asm = row.match(/attackSpeedMultiplier = (-?\d+)/)?.[1]; if (asm) put("asMore", +asm);
      // Stats the speed check needs, by name.
      for (const [stat, key] of [["support_reduced_duration_skill_effect_duration_+%_final", "durMore"], ["attack_speed_+%_granted_from_skill", "auraAs"],
        ["attack_speed_+%", "as"], ["support_faster_attacks_attack_speed_+%", "as"], ["skill_effect_duration_+%", "durInc"], ["support_more_duration_skill_effect_duration_+%_final", "durMore"]]) {
        const i = statAt(stat); if (i >= 0 && nums[i] !== undefined) put(key, nums[i]);
      }
    }
    const extraProj = b.match(/\{ "number_of_additional_projectiles", (\d+) \}/)?.[1]; if (extraProj) g.proj = +extraProj;
    const spQ = b.match(/qualityStats = \{[^}]*?\{ "base_number_of_spectres_allowed", ([\d.]+) \}/)?.[1]; if (spQ) g.spectresQ = +spQ;
    const baseDur = b.match(/\{ "base_skill_effect_duration", (\d+) \}/)?.[1]; if (baseDur) g.dur = +baseDur / 1000;
    const delay = b.match(/\{ "kinetic_fusillade_base_delay_between_projectiles_ms", (\d+) \}/)?.[1]; if (delay) g.projDelay = +delay / 1000;
    if (/SkillType\.Aura\] = true/.test(b) || /HasReservation\] = true/.test(b)) g.reserves = true;
    if (gems[name]) { for (const [k, v] of Object.entries(g)) { const o = gems[name][k]; if (o === undefined || (Array.isArray(o) && !o.length)) gems[name][k] = v; } }
    else gems[name] = g;
  }
}

// Level tables with gaps (a stat missing at some levels): fill each gap from the level before, so every list is complete.
for (const g of Object.values(gems)) for (const [k, v] of Object.entries(g)) if (Array.isArray(v)) { for (let i = 0; i < v.length; i++) if (v[i] == null) v[i] = i ? v[i - 1] : (v.find((x) => x != null) ?? 0); }

// ---- item bases
const bases = {};
for (const f of await list("Bases")) {
  const t = await get(RAW + "Bases/" + f);
  for (const m of t.matchAll(/itemBases\["([^"]+)"\] = \{([\s\S]*?)\n\}/g)) {
    const r = m[2].match(/req = \{([^}]*)\}/)?.[1] ?? "";
    const n = (k) => +(r.match(new RegExp(k + " = (\\d+)"))?.[1] ?? 0);
    const aps = m[2].match(/AttackRateBase = ([\d.]+)/)?.[1];
    const two = /twohand = true/.test(m[2]);
    bases[m[1]] = [n("level"), n("str"), n("dex"), n("int"), aps ? +aps : 0, two ? 1 : 0];
  }
}

// ---- passive tree
const tree = JSON.parse(await get("https://raw.githubusercontent.com/grindinggear/skilltree-export/master/data.json"));
const C = tree.constants;
const ang = (n, i) => n === 16 ? [0, 30, 45, 60, 90, 120, 135, 150, 180, 210, 225, 240, 270, 300, 315, 330][i]
  : n === 40 ? [0, 10, 20, 30, 40, 45, 50, 60, 70, 80, 90, 100, 110, 120, 130, 135, 140, 150, 160, 170, 180, 190, 200, 210, 220, 225, 230, 240, 250, 260, 270, 280, 290, 300, 310, 315, 320, 330, 340, 350][i]
  : 360 * i / n;
const WANT = /Strength|Dexterity|Intelligence|Attributes|Skill Effect Duration|Attack Speed|maximum Mana|Reservation Efficiency|Mana Cost|Leeched as Mana|Spectre|Minion Skill Gems|Minion Gems/;
const nodes = {};
for (const [id, n] of Object.entries(tree.nodes)) {
  if (id === "root" || n.group == null) continue;
  const g = tree.groups[n.group];
  const r = C.orbitRadii[n.orbit], a = ang(C.skillsPerOrbit[n.orbit], n.orbitIndex) * Math.PI / 180;
  const stats = (n.stats ?? []).filter((s) => WANT.test(s));
  const mast = n.isMastery ? Object.fromEntries((n.masteryEffects ?? []).map((e) => [e.effect, e.stats.filter((s) => WANT.test(s))]).filter(([, s]) => s.length)) : null;
  const o = { x: Math.round(g.x + Math.sin(a) * r), y: Math.round(g.y - Math.cos(a) * r) };
  if (stats.length) o.s = stats;
  if (mast && Object.keys(mast).length) o.m = mast;
  if (n.isJewelSocket) o.j = 1;
  if (n.ascendancyName) o.a = 1;
  if (n.classStartIndex !== undefined) o.c = 1;
  if (n.isNotable || n.isKeystone || n.isMastery || n.isJewelSocket) o.n = 1; // not a "small" passive (Unnatural Instinct, Might of the Meek)
  if (o.s || o.m || o.j || o.a || o.c) nodes[id] = o; // positions matter for nodes with stats (jewel radius) and sockets
  else if (!o.a && !o.c) nodes[id] = 0; // still counted as a point
}
const out = { gems, bases, nodes };
const file = new URL("../core/data/poe1_buildcheck.json", import.meta.url);
fs.writeFileSync(file, JSON.stringify(out));
console.log(Object.keys(gems).length, "gems,", Object.keys(bases).length, "bases,", Object.keys(nodes).length, "nodes,", fs.statSync(file).size, "bytes");

// ---- spectre names (28 Nov 2026, he asked which spectres a build uses): PoB's Spectres.lua, id -> monster name.
// A Raise Spectre gem in a PoB build names its monster by id ("Metadata/Monsters/BloodChieftain/MonkeyChiefBloodEnrage").
const spectres = {};
for (const m of (await get(RAW + "Spectres.lua")).matchAll(/minions\["([^"]+)"\] = \{\s*name = "([^"]+)"/g)) spectres[m[1]] = m[2];
const sfile = new URL("../core/data/poe1_spectres.json", import.meta.url);
fs.writeFileSync(sfile, JSON.stringify(spectres));
console.log(Object.keys(spectres).length, "spectres,", fs.statSync(sfile).size, "bytes");
