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
const round = (x) => Math.round(x * 1000) / 1000;
const all = {}, mechs = {};
let total = 0;
for (const t of trees) {
  total += t.w;
  for (const i of t.S) all[i] = (all[i] ?? 0) + t.w;
  for (const m of t.farms) {
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
}).replace(/("[a-z ]+":\{"w")/g, "\n$1") + "\n");
console.log(`${trees.length} trees, ${Object.keys(mechs).length} mechanics -> ${out}`);
