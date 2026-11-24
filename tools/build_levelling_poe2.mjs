// Builds core/data/levelling_poe2.json (the levelling window's PoE 2 campaign route) from Exile UI's PoE 2 leveling
// guide (Lailloken/Exile-UI, MIT: data/english/[leveltracker] default guide 2.json + areas 2.json).
//
//   node tools/build_levelling_poe2.mjs <guide 2.json> <areas 2.json>
//
// Same shape as levelling.json (PoE 1, from exile-leveling): areas, steps with (kind, value) parts and hint subs.
// Their markup: "(img:waypoint) to areaidg1_4 ;; the grelwood", "(hint)_ ...", "(color:red)...", "(quest:3_obelisks)",
// line prefixes "optional:", "leaguestart:" (league start only) and "twinkrun:" (speedruns with twinked gear: left out).
import fs from "node:fs";
import path from "node:path";

const [guideFile, areasFile] = process.argv.slice(2);
if (!guideFile || !areasFile) { console.error("usage: node tools/build_levelling_poe2.mjs <guide 2.json> <areas 2.json>"); process.exit(1); }
const guide = JSON.parse(fs.readFileSync(guideFile, "utf8"));
const areasRaw = JSON.parse(fs.readFileSync(areasFile, "utf8"));

const title = (s) => s.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()).replace(/\b(Of|The|To|In|And)\b/g, (w) => w.toLowerCase()).replace(/^./, (c) => c.toUpperCase());
const areas = {};
areasRaw.forEach((list, i) => {
  for (const a of list) {
    const lvl = Math.round(Number(String(a.recommendation ?? "").split("|")[0])) || 0;
    areas[a.id.toLowerCase()] = { n: title(a.name), a: i + 1, l: lvl, t: a.id.includes("town") };
  }
});

const IMG = { waypoint: "waypoint", portal: "portal", checkpoint: "checkpoint", quest_2: "", quest: "", skill: "skill gem", skill2: "skill gem",
  support: "support gem", support2: "support gem", "in-out2": "", "in-out": "", ring: "ring", rune: "rune", exa: "exalt", jeweller: "jeweller's orb" };

/** One guide line -> [parts, plain text]. */
function render(line) {
  let s = line;
  const parts = [];
  // Where it takes you: the last area id on the line.
  const ids = [...s.matchAll(/areaid(\w+)/g)].map((m) => m[1].toLowerCase());
  s = s.replace(/;;[^|,]*?(?=(\|\||,|$))/g, "");
  s = s.replace(/areaid(\w+)/g, (_, id) => areas[id.toLowerCase()]?.n ?? id);
  s = s.replace(/\(img:([\w-]+)\)/g, (_, k) => (IMG[k] ?? k.replace(/_/g, " ")));
  s = s.replace(/\(color:[^)]+\)/g, "").replace(/\(quest:([^)]+)\)/g, (_, q) => q.replace(/_/g, " "));
  s = s.replace(/\(hint\)_?\s*-?\s*/g, "").replace(/[<>]/g, "").replace(/_/g, " ").replace(/\|\|/g, "·").replace(/\s+/g, " ").trim();
  const last = ids[ids.length - 1];
  if (last && areas[last]) {
    const kind = /\(img:waypoint\) to areaid/.test(line) ? "waypoint" : "enter";
    // The words before the zone, then the zone (coloured by the window).
    const name = areas[last].n;
    const i = s.lastIndexOf(name);
    if (i >= 0) { parts.push(["text", s.slice(0, i)], [kind, last]); if (s.slice(i + name.length).trim()) parts.push(["text", s.slice(i + name.length)]); }
    else parts.push(["text", s + " "], [kind, last]);
  } else parts.push(["text", s]);
  return parts;
}

const steps = [];
guide.forEach((act, ai) => {
  for (const entry of act) {
    // A page is a list of lines, or { condition: ["league-start", "yes"|"no"], lines } for league-start-only pages.
    const page = Array.isArray(entry) ? entry : entry.lines;
    const pageCond = Array.isArray(entry) ? [] : entry.condition[0] === "league-start" ? [entry.condition[1] === "no" ? "!LEAGUE_START" : "LEAGUE_START"] : [];
    let skipHints = false;
    for (const raw of page) {
      let line = raw.trim();
      if (!line) continue;
      const hint = line.startsWith("(hint)");
      // Notes about Exile UI's own settings ("info: optional rewards…") and their hints: not for us.
      if (/^(\(color:\w+\))?info:/i.test(line)) { skipHints = true; continue; }
      if (hint && skipHints) continue;
      skipHints = false;
      let cond = [...pageCond];
      if (/^twinkrun:/i.test(line)) continue;
      if (/^leaguestart:/i.test(line)) { if (!cond.includes("LEAGUE_START")) cond.push("LEAGUE_START"); line = line.replace(/^leaguestart:\s*/i, ""); }
      const parts = render(line);
      const text = parts.map((p) => p[0] === "text" ? p[1] : areas[p[1]]?.n ?? p[1]).join("").trim();
      if (!text) continue;
      if (hint && steps.length) { steps[steps.length - 1].subs.push(text); continue; }
      const step = { act: ai + 1, parts, subs: [] };
      if (cond.length) step.cond = cond;
      steps.push(step);
    }
  }
});

const out = { credit: "PoE 2 route and zones from Exile UI's leveling guide by Lailloken (MIT).", areas, quests: {}, gems: [], steps };
const dest = path.join(path.dirname(new URL(import.meta.url).pathname), "..", "core", "data", "levelling_poe2.json");
fs.writeFileSync(dest, JSON.stringify(out));
console.log(`steps ${steps.length}, areas ${Object.keys(areas).length} -> ${dest}`);
