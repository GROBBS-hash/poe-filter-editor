// What people who farm each mechanic take on their atlas, as shares (no trees are shipped), for the atlas builder
// (core/src/atlas_opt.rs: it keeps the builder close to what experts do where the maths has no number).
//
//   node tools/build_atlas_shares.mjs [out.json] [players]
//
// Reads core/data/atlas_meta.json (poe.ninja's most copied trees) and the creator plans kept on GROBBS's PC in
// .claude/atlas-research/data (not in git). Near-identical trees count as one idea. "players" leaves the creators out, for
// testing against them (`cargo test -p filter-core --lib atlas_opt::score -- --ignored --nocapture`).
import fs from "node:fs";
import { createRequire } from "node:module";
const L = createRequire(import.meta.url)("../.claude/atlas-research/data/dm/lib.cjs");
const [out = "core/data/atlas_shares.json", only] = process.argv.slice(2);

const trees = L.dataset().filter((t) => t.legal && (only !== "players" || t.src === "player"));
// Trees creators published as links (decoded by the research), each one idea unless it's a near-copy of one above.
const read = (n) => JSON.parse(fs.readFileSync(new URL(`../.claude/atlas-research/data/oc/${n}_decoded.json`, import.meta.url)));
if (only !== "players") for (const [id, nodes] of [...Object.entries(read("pp")), ...Object.entries(read("blush"))]) {
  const S = new Set([L.START, ...nodes]);
  if (S.size < 120 || S.size > 151 || trees.some((t) => L.jac(t.S, S) >= 0.75 && t.src !== "player" && t.id.startsWith("oc:"))) continue;
  trees.push({ id: "oc:" + id, src: "creator", author: id, name: id, S, w: 0.8, farms: L.farms(S) });
}
// Goals that aren't a mechanic: which trees are for them (by what only such trees take, or named).
const has = (t, ...a) => a.every((x) => t.S.has(x));
const anomalies = Object.keys(L.fam).filter((i) => L.fam[i] === "anomalies" && !L.isN(i)).map(Number);
const named = { destructive: ["oc:64HK"], rush: ["oc:64NP", "oc:64zt"], currency: ["oc:fubgun", "oc:fubgunMirage", "oc:kingpin", "oc:64UJ"], threads: ["oc:64Tz"] };
const farm = (t, ...m) => t.farms.some((x) => m.includes(x));
const GOALS = {
  destructive: (t) => has(t, 34384) && !farm(t, "betrayal"),
  terror: (t) => !has(t, 34384) && has(t, 58232, 9780) && (has(t, 18476) || has(t, 12651) || has(t, 48336)) && !farm(t, "betrayal", "legion"),
  rush: (t) => !has(t, 34384) && !farm(t, "betrayal", "legion", "abyss", "breach", "blight", "ritual") && ((has(t, 64222) && anomalies.filter((i) => t.S.has(i)).length >= 4) || (t.id.includes("n1cm70gm"))),
  currency: (t) => has(t, 13833, 30266, 34393, 33008) && farm(t, "legion", "bestiary"),
  threads: (t) => has(t, 61484, 64462),
  maps: (t) => has(t, 43903) && (has(t, 35608) || has(t, 24609)) && !farm(t, "essence", "incursion", "betrayal", "settlers", "memories", "mercenaries"),
};
const round = (x) => Math.round(x * 1000) / 1000;
const all = {}, mechs = {};
let total = 0;
for (const t of trees) {
  total += t.w;
  for (const i of t.S) all[i] = (all[i] ?? 0) + t.w;
  const goals = Object.keys(GOALS).filter((g) => GOALS[g](t) || named[g]?.includes(t.id)).map((g) => "goal:" + g);
  for (const m of [...t.farms, ...goals]) {
    const c = (mechs[m] ??= { w: 0, n: {} });
    c.w += t.w;
    for (const i of t.S) c.n[i] = (c.n[i] ?? 0) + t.w;
  }
}
const shares = (n, w) => Object.fromEntries(Object.entries(n).map(([i, x]) => [i, round(x / w)]).filter(([, x]) => x >= 0.02));
const family = {};
for (const i in L.fam) if (!["start", "gateway", "other"].includes(L.fam[i])) (family[L.fam[i]] ??= []).push(+i);
fs.writeFileSync(out, JSON.stringify({
  about: "share of trees taking each node: all trees, and per mechanic those that farm it (w = independent ideas, weighted); family = what each node is about",
  built: new Date().toISOString().slice(0, 10), trees: trees.length,
  family, general: [...L.GENERIC], all: shares(all, total),
  mechs: Object.fromEntries(Object.entries(mechs).map(([m, c]) => [m, { w: round(c.w), n: shares(c.n, c.w) }])),
}).replace(/("[a-z: ]+":\{"w")/g, "\n$1") + "\n");
for (const g in GOALS) console.log(`  ${g}: ${trees.filter((t) => GOALS[g](t) || named[g]?.includes(t.id)).map((t) => t.id.slice(0, 24)).join(", ")}`);
console.log(`${trees.length} trees, ${Object.keys(mechs).length} mechanics -> ${out}`);
