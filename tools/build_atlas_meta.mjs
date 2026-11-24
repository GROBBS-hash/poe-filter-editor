// What players run on their atlas, for the farm finder: poe.ninja's atlas trees (100,000 per league), read once by hand
// into core/data/atlas_meta.json and shipped with the app (the app never asks poe.ninja itself, see fetch_prices.py).
//
//   node tools/build_atlas_meta.mjs <snapshot> [league url]      e.g. 0747-20261124-14312 covens
//
// The snapshot number is in the page's own requests (open poe.ninja/poe1/atlas-trees/covens in a browser and look at the
// ".../api/atlas-trees/<snapshot>/search" request). poe.ninja turns scripts away from its pages (24 Nov: 403), so this
// only reads the open data feed and can't run on its own in the price job. Refresh before a release now and then.
// One request every 1.5 s.
//
// The answers are protobuf (no public schema), so `decode` only trusts what it can check: the names list must fill its
// text exactly, and every mechanic filter must come back with that mechanic in (nearly) every tree.
import fs from "node:fs";

const UA = "poe-filter-editor (github.com/GROBBS-hash/poe-filter-editor)";
/** Mechanics (poe.ninja's names) whose own trees are read: what people pair them with. */
const MECHS = ["Abyss", "Bestiary", "Betrayal", "Blight", "Breach", "Expedition", "Incursion", "Legion", "Ritual", "Ultimatum",
  "Wildwood", "Essence", "Delirium"];
/** Trees kept per mechanic. */
const TREES = 12;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let asked = 0;
async function get(url) {
  if (asked++) await sleep(1500);
  const r = await fetch(url, { headers: { "User-Agent": UA }, signal: AbortSignal.timeout(60000) });
  if (!r.ok) throw new Error(`${url}: ${r.status}`);
  return Buffer.from(await r.arrayBuffer());
}

/** A protobuf message's fields: [number, value] (numbers for varints, Buffers for anything with a length). */
function fields(b) {
  const out = [];
  let o = 0;
  const varint = () => { let v = 0, s = 1, c; do { if (o >= b.length) throw new Error("cut off"); c = b[o++]; v += (c & 127) * s; s *= 128; } while (c & 128); return v; };
  while (o < b.length) {
    const k = varint(), n = Math.floor(k / 8), w = k & 7;
    if (w === 0) out.push([n, varint()]);
    else if (w === 2) { const l = varint(); if (o + l > b.length) throw new Error("cut off"); out.push([n, b.subarray(o, o + l)]); o += l; }
    else if (w === 5) { out.push([n, b.readFloatLE(o)]); o += 4; }
    else if (w === 1) { out.push([n, b.readDoubleLE(o)]); o += 8; }
    else throw new Error("unknown field");
  }
  return out;
}
const varints = (b) => { const out = []; let v = 0, s = 1; for (const c of b) { v += (c & 127) * s; s *= 128; if (!(c & 128)) { out.push(v); v = 0; s = 1; } } return out; };
const first = (f, n) => f.find((x) => x[0] === n)?.[1];
const text = (f, n) => first(f, n)?.toString("utf8") ?? "";

/** One search answer: how many trees, how many have each mechanic (by number), and its most copied trees. */
function decode(buf) {
  let top = fields(buf);
  if (top.length === 1 && Buffer.isBuffer(top[0][1])) top = fields(top[0][1]);
  const total = first(top, 1) ?? 0;
  const counts = {};
  const facet = top.filter((x) => x[0] === 2).map((x) => fields(x[1])).find((f) => text(f, 1) === "mechanics");
  for (const [n, v] of facet ?? []) if (n === 3) { const f = fields(v); counts[first(f, 1) ?? 0] = first(f, 2) ?? 0; }
  const dict = top.filter((x) => x[0] === 6).map((x) => fields(x[1])).find((f) => text(f, 1) === "mechanics");
  const cols = Object.fromEntries(top.filter((x) => x[0] === 12).map((x) => { const f = fields(x[1]); return [text(f, 1), f]; }));
  const rows = (name) => (cols[name] ?? []).filter((x) => x[0] === 9).map((x) => { const f = fields(x[1]); return f.length ? varints(f[0][1]) : []; });
  const plain = (name) => { const b = first(cols[name] ?? [], 6); return b ? varints(b) : []; };
  const pop = plain("popularity"), nodes = rows("treeid"), names = rows("mechanics.names"), points = rows("mechanics.counts");
  const trees = nodes.map((n, i) => ({ copies: pop[i] ?? 1, nodes: n, mechs: (names[i] ?? []).map((m, j) => [m, points[i]?.[j] ?? 0]) }));
  return { total, counts, trees, dicts: dict ? [text(dict, 2), text(dict, 3)] : [] };
}

/** The mechanics' names in poe.ninja's dictionary file: a list of lengths followed by the names run together. Found by
 *  looking for the one spot where `n` lengths add up to exactly the text after them. */
export function names(buf, n) {
  for (let at = 0; at + n < buf.length; at++) {
    const lens = [...buf.subarray(at, at + n)];
    const sum = lens.reduce((a, b) => a + b, 0);
    if (lens.some((l) => l < 3 || l > 40) || at + n + sum > buf.length || buf.length - (at + n + sum) > 8) continue;
    const blob = buf.subarray(at + n, at + n + sum).toString("latin1");
    if (!/^[A-Za-z&' -]+$/.test(blob)) continue;
    let o = 0;
    const out = lens.map((l) => blob.slice(o, o += l));
    if (out.every((x) => /^[A-Z]/.test(x))) return out;
  }
  throw new Error("couldn't read the mechanics' names");
}

async function build(version, url) {
  const state = JSON.parse(await get("https://poe.ninja/poe1/api/data/atlas-tree-index-page-state"));
  const league = state.leagues.find((l) => l.leagueUrl === url);
  if (!league) throw new Error(`no league "${url}"`);
  const search = async (m) => decode(await get(`https://poe.ninja/poe1/api/atlas-trees/${version}/search?${m ? `mechanics=${encodeURIComponent(m)}&` : ""}overview=${league.leagueUrl}`));
  const all = await search("");
  const n = Math.max(...Object.keys(all.counts).map(Number)) + 1;
  let list = null;
  for (const h of all.dicts.filter(Boolean)) {
    try { list = names(await get(`https://poe.ninja/api/lookup/poe1/atlas-trees/dictionary/${h}`), n); break; } catch (e) { console.error(`  dictionary ${h}: ${e.message}`); }
  }
  if (!list) throw new Error("no names");
  const named = (counts, total) => Object.fromEntries(Object.entries(counts).map(([i, c]) => [list[i], +(c / total).toFixed(4)]).filter(([k]) => k));
  const tidy = (t) => ({ copies: t.copies, mechs: t.mechs.map(([i, p]) => [list[i], p]), nodes: t.nodes });
  const out = { league: league.leagueName, updated: new Date().toISOString().slice(0, 19) + "Z", total: all.total,
    share: named(all.counts, all.total), with: {}, trees: {} };
  out.trees.All = all.trees.slice(0, TREES).map(tidy);
  for (const m of MECHS.filter((x) => list.includes(x))) {
    const r = await search(m);
    const w = named(r.counts, r.total);
    if (!(w[m] > 0.99)) throw new Error(`${m}: the filter didn't filter (names out of step?)`);
    delete w[m];
    out.with[m] = { total: r.total, share: w };
    out.trees[m] = r.trees.slice(0, TREES).map(tidy);
    console.log(`  ${m}: ${r.total} trees`);
  }
  return out;
}

const [version, url = "covens"] = process.argv.slice(2);
if (!/^\d{4}-\d{8}-\d+$/.test(version ?? "")) { console.error("usage: node tools/build_atlas_meta.mjs <snapshot> [league url]"); process.exit(1); }
const out = await build(version, url);
fs.writeFileSync(new URL("../core/data/atlas_meta.json", import.meta.url), JSON.stringify(out));
console.log(`${out.total} trees`);
