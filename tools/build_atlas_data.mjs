// Builds the atlas passive tree data the app ships with (PoE 1), like tools/build_tree_data.py does for the
// character tree.
//
//     public/atlas_poe1.json
//     {
//       "version": "3.30.0",
//       "bounds": [min_x, min_y, max_x, max_y],
//       "start": id,                              // the centre node every path starts from
//       "nodes": {id: [x, y, kind, name, [stats]]},
//       "edges": [[a, b] | [a, b, cx, cy, r] | [a, b, 0]],  // arc with centre + radius; [a, b, 0] = not drawn (gateways)
//       "labels": [[x, y, name]],                 // area names (the big background icons)
//       "choices": {node: {effect: text}},       // multiple-choice notables (e.g. Blended Realities)
//     }
//     kind: 0 small, 1 notable, 2 keystone, 4 gateway, 5 start
//
// Source: GGG's official export, https://github.com/grindinggear/atlastree-export (data.json).
// Usage: node tools/build_atlas_data.mjs <path to data.json> <version> [out]

import fs from "node:fs";

const ANGLES_16 = [0, 30, 45, 60, 90, 120, 135, 150, 180, 210, 225, 240, 270, 300, 315, 330];
const ANGLES_40 = [0, 10, 20, 30, 40, 45, 50, 60, 70, 80, 90, 100, 110, 120, 130, 135, 140, 150, 160, 170, 180, 190, 200,
  210, 220, 225, 230, 240, 250, 260, 270, 280, 290, 300, 310, 315, 320, 330, 340, 350];
const rad = (d) => (d * Math.PI) / 180;
const angle = (slots, i) => slots === 16 ? rad(ANGLES_16[i]) : slots === 40 ? rad(ANGLES_40[i]) : (2 * Math.PI * i) / slots;
const r1 = (v) => Math.round(v * 10) / 10;
const clean = (s) => s.replace(/\[([^\]|]+)\|([^\]]+)\]/g, "$2").replace(/\[([^\]]+)\]/g, "$1");

const [src, version, out = "public/atlas_poe1.json"] = process.argv.slice(2);
if (!src || !version) { console.error("usage: node tools/build_atlas_data.mjs data.json 3.30.0 [out]"); process.exit(1); }
const d = JSON.parse(fs.readFileSync(src, "utf8"));
const radii = d.constants.orbitRadii, per = d.constants.skillsPerOrbit;
const nodes = {}, info = {}, labels = [], choices = {};
const start = Number(d.nodes.root.out[0]);
for (const [id, n] of Object.entries(d.nodes)) {
  if (id === "root" || n.group == null) continue;
  const g = d.groups[n.group];
  const r = radii[n.orbit], a = angle(per[n.orbit], n.orbitIndex);
  const x = g.x + r * Math.sin(a), y = g.y - r * Math.cos(a);
  if (n.isMastery) { if (n.name) labels.push([r1(x), r1(y), n.name]); continue; }
  const kind = Number(id) === start ? 5 : n.isWormhole ? 4 : n.isKeystone ? 2 : n.isNotable ? 1 : 0;
  nodes[id] = [r1(x), r1(y), kind, n.name || "", (n.stats || []).map(clean)];
  info[id] = { group: n.group, orbit: n.orbit, x, y };
  if (n.isMultipleChoiceVariant) {
    choices[id] = Object.fromEntries((n.variantEffects || []).map((e) => [e.effect, e.stats.map(clean).join(" / ")]));
  }
}

const edges = [], seen = new Set();
for (const [id, n] of Object.entries(d.nodes)) {
  if (!nodes[id]) continue;
  const curve = Object.fromEntries((n.connections || []).map((c) => [c.to, c]));
  for (const o of [...(n.out || []), ...(n.in || [])]) {
    if (!nodes[o]) continue;
    const key = [id, o].sort().join("-");
    if (seen.has(key)) continue;
    seen.add(key);
    const a = info[id], b = info[o];
    if (nodes[id][2] === 4 && nodes[o][2] === 4) { edges.push([+id, +o, 0]); continue; } // gateway pair: no line
    const c = curve[o] || (d.nodes[o].connections || []).find((x) => x.to === id);
    if (c && c.orbit) {
      // A curved line round a given centre (only if both ends really sit on that circle).
      const r = radii[Math.abs(c.orbit)];
      const da = Math.hypot(a.x - c.orbitX, a.y - c.orbitY), db = Math.hypot(b.x - c.orbitX, b.y - c.orbitY);
      if (Math.abs(da - r) < 2 && Math.abs(db - r) < 2) { edges.push([+id, +o, r1(c.orbitX), r1(c.orbitY), r]); continue; }
    }
    if (a.group === b.group && a.orbit === b.orbit && a.orbit !== 0) {
      const g = d.groups[a.group];
      edges.push([+id, +o, r1(g.x), r1(g.y), radii[a.orbit]]);
    } else edges.push([+id, +o]);
  }
}

const xs = Object.values(nodes).map((v) => v[0]), ys = Object.values(nodes).map((v) => v[1]);
const data = { version, bounds: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)], start, nodes, edges, labels, choices };
fs.writeFileSync(out, JSON.stringify(data));
console.log(Object.keys(nodes).length, "nodes,", edges.length, "edges,", labels.length, "labels,", Object.keys(choices).length, "choice nodes");
