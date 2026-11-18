"""Builds the passive tree drawing data the app ships with (PoE 1).

    public/tree_poe1.json
    {
      "bounds": [min_x, min_y, max_x, max_y],
      "nodes": {id: [x, y, kind, name, [stats], ascendancy or ""]},
      "edges": [[a, b] | [a, b, cx, cy, r]],   # with a centre + radius when the line is an arc
    }
    kind: 0 small, 1 notable, 2 keystone, 3 mastery, 4 jewel socket, 5 class start, 6 ascendancy start

Source: GGG's official export, https://github.com/grindinggear/skilltree-export (data.json).
Usage: python tools/build_tree_data.py <path to data.json> [out]
"""

import json
import math
import sys

# Orbits with 16 and 40 slots aren't evenly spaced (same tables the game and PoB use).
ANGLES_16 = [0, 30, 45, 60, 90, 120, 135, 150, 180, 210, 225, 240, 270, 300, 315, 330]
ANGLES_40 = [0, 10, 20, 30, 40, 45, 50, 60, 70, 80, 90, 100, 110, 120, 130, 135, 140, 150, 160, 170, 180, 190, 200,
             210, 220, 225, 230, 240, 250, 260, 270, 280, 290, 300, 310, 315, 320, 330, 340, 350]


def angle(slots, index):
    if slots == 16:
        return math.radians(ANGLES_16[index])
    if slots == 40:
        return math.radians(ANGLES_40[index])
    return 2 * math.pi * index / slots


def main(src, out):
    d = json.load(open(src, encoding="utf-8"))
    radii = d["constants"]["orbitRadii"]
    per = d["constants"]["skillsPerOrbit"]
    groups = d["groups"]
    nodes, pos, info = {}, {}, {}
    effects = {}  # mastery effect id -> its text (what you picked on a mastery)
    for nid, n in d["nodes"].items():
        if nid == "root" or "group" not in n or n.get("isProxy") or n.get("expansionJewel", {}).get("parent"):
            continue
        g = groups[str(n["group"])]
        r = radii[n["orbit"]]
        a = angle(per[n["orbit"]], n["orbitIndex"])
        x, y = g["x"] + r * math.sin(a), g["y"] - r * math.cos(a)
        kind = (6 if n.get("isAscendancyStart") else 5 if "classStartIndex" in n else 2 if n.get("isKeystone")
                else 3 if n.get("isMastery") else 4 if n.get("isJewelSocket") else 1 if n.get("isNotable") else 0)
        name = n.get("name", "")
        stats = n.get("stats", [])
        if n.get("isMastery"):
            for e in n.get("masteryEffects", []):
                effects[str(e["effect"])] = " / ".join(e.get("stats", []))
            stats = [" / ".join(e.get("stats", [])) for e in n.get("masteryEffects", [])]
        nodes[nid] = [round(x, 1), round(y, 1), kind, name, stats, n.get("ascendancyName", "")]
        pos[nid] = (x, y)
        info[nid] = (n["group"], n["orbit"], n.get("ascendancyName", ""), kind)

    edges, seen = [], set()
    for nid, n in d["nodes"].items():
        if nid not in nodes:
            continue
        for o in n.get("out", []) + n.get("in", []):
            if o not in nodes or (min(nid, o), max(nid, o)) in seen:
                continue
            seen.add((min(nid, o), max(nid, o)))
            ga, oa, asc_a, ka = info[nid]
            gb, ob, asc_b, kb = info[o]
            if asc_a != asc_b or 3 in (ka, kb) or 5 in (ka, kb) and 5 in (ka, kb) and ka == kb:
                continue  # no lines from ascendancy to main tree, or to masteries
            if ga == gb and oa == ob and oa != 0:
                g = groups[str(ga)]
                edges.append([int(nid), int(o), round(g["x"], 1), round(g["y"], 1), radii[oa]])
            else:
                edges.append([int(nid), int(o)])

    xs = [v[0] for v in nodes.values() if not v[5]]
    ys = [v[1] for v in nodes.values() if not v[5]]
    data = {"bounds": [min(xs), min(ys), max(xs), max(ys)], "nodes": nodes, "edges": edges,
            "classes": [c["name"] for c in d["classes"]], "effects": effects}
    json.dump(data, open(out, "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    print(len(nodes), "nodes,", len(edges), "edges")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else "public/tree_poe1.json")
