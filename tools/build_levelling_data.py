"""Builds the levelling-gear data the app ships with (PoE 1):

    core/data/poe1_uniques.json  [{name, base, class, level, mods, stats}]
    core/data/poe1_gems.json     {gem name: [stat tags it cares about]}

Sources (both open data):
  - Path of Building Community: src/Data/Uniques/*.lua (unique text) and src/Data/Gems.lua (gem tags)
  - RePoE: data/base_items.json (base required levels)

Usage: python tools/build_levelling_data.py <PathOfBuilding checkout> <repoe base_items.json>
Standard library only.
"""

import glob
import json
import os
import re
import sys

# Stat tags, matched against a unique's mod text (lower case). Order = display order.
STATS = [
    ("gem_levels", "+ gem levels", [r"to level of (all|socketed)"]),
    ("spell_damage", "Spell damage", [r"spell damage", r"damage to spells", r"increased damage with spells"]),
    ("cast_speed", "Cast speed", [r"cast speed"]),
    ("attack_speed", "Attack speed", [r"attack speed"]),
    ("added_attack", "Added damage to attacks", [r"adds .* damage to attacks", r"adds .* damage$", r"physical damage$"]),
    ("crit", "Critical strikes", [r"critical strike"]),
    ("minion", "Minions", [r"minion", r"zombie", r"skeleton", r"spectre", r"golem"]),
    ("projectile", "Projectiles", [r"projectile", r"arrow"]),
    ("area", "Area of effect", [r"area of effect", r"area damage"]),
    ("fire", "Fire", [r"\bfire\b", r"ignite", r"burning"]),
    ("cold", "Cold", [r"\bcold\b", r"freeze", r"chill"]),
    ("lightning", "Lightning", [r"lightning", r"shock"]),
    ("chaos", "Chaos", [r"\bchaos\b", r"poison"]),
    ("physical", "Physical", [r"physical damage", r"bleed"]),
    ("elemental", "Elemental", [r"elemental damage"]),
    ("dot", "Damage over time", [r"damage over time"]),
    ("totem_trap_mine", "Totems, traps & mines", [r"totem", r"\btrap", r"\bmine"]),
    ("life", "Life", [r"maximum life", r"to maximum life", r"life regenerat", r"life on hit", r"leech"]),
    ("energy_shield", "Energy shield", [r"energy shield"]),
    ("mana", "Mana", [r"\bmana\b"]),
    ("resistances", "Resistances", [r"resistance"]),
    ("movement_speed", "Movement speed", [r"movement speed"]),
    ("experience", "Experience", [r"experience"]),
    ("links", "Linked sockets", []),  # set from the item's fixed sockets (e.g. Tabula Rasa)
]
# Lines that mention damage but don't help you deal it.
NOT_OFFENCE = re.compile(r"reflect|taken|against you|enemies deal|you take")

# What a gem's tags make worth having.
GEM_TAG_STATS = {
    "spell": ["gem_levels", "spell_damage", "cast_speed"],
    "attack": ["attack_speed", "added_attack"],
    "melee": ["attack_speed", "added_attack", "physical"],
    "bow": ["attack_speed", "projectile", "added_attack"],
    "projectile": ["projectile"],
    "area": ["area"],
    "minion": ["minion", "gem_levels"],
    "fire": ["fire", "elemental"],
    "cold": ["cold", "elemental"],
    "lightning": ["lightning", "elemental"],
    "chaos": ["chaos"],
    "physical": ["physical"],
    "totem": ["totem_trap_mine"],
    "trap": ["totem_trap_mine"],
    "mine": ["totem_trap_mine"],
    "critical": ["crit"],
}

PoB_CLASS = {  # PoB file -> fallback only; the real class comes from the base
}


def clean(line):
    line = re.sub(r"\{[^}]*\}", "", line).strip()
    return line


def parse_uniques(pob):
    out = []
    for path in sorted(glob.glob(os.path.join(pob, "src/Data/Uniques/*.lua"))):
        text = open(path, encoding="utf-8").read()
        for block in re.findall(r"\[\[(.*?)\]\]", text, re.S):
            lines = [l.strip() for l in block.strip().splitlines() if l.strip()]
            if len(lines) < 2:
                continue
            name, base = lines[0], lines[1]
            meta, mods = {}, []
            variants = []
            for l in lines[2:]:
                m = re.match(r"^([A-Z][A-Za-z ]+): (.*)$", l)
                if m and m.group(1) in ("Variant", "Source", "League", "Implicits", "Limited to", "Radius", "Upgrade",
                                        "Version", "Has Alt Variant", "Selected Variant", "Selected Variant Group",
                                        "Crafted", "Selected Alt Variant", "Has Alt Variant Two", "Talisman Tier",
                                        "Sockets", "Selected Alt Variant Two", "LevelReq"):
                    if m.group(1) == "Variant":
                        variants.append(m.group(2))
                    meta[m.group(1)] = m.group(2)
                    continue
                mods.append(l)
            if "No longer obtainable" in meta.get("Source", "") or name.startswith("Replica ") or "Drops from unique{" in meta.get("Source", "") and False:
                continue
            # Keep lines for the newest variant (the last one listed is "Current").
            cur = len(variants)
            kept = []
            level = 0
            for l in mods:
                vm = re.match(r"^\{variant:([\d,]+)\}", l)
                if vm and cur and str(cur) not in vm.group(1).split(","):
                    continue
                c = clean(l)
                lm = re.match(r"^Requires Level (\d+)", c)
                if lm:
                    level = int(lm.group(1))
                    continue
                if c:
                    kept.append(c)
            out.append({"name": name, "base": base, "level": level, "mods": kept, "linked": "-" in meta.get("Sockets", "")})
    return out


def main(pob, base_items_path, out_dir):
    bases = json.load(open(base_items_path, encoding="utf-8"))
    base_req = {}
    for v in bases.values():
        if v.get("domain") != "item":
            continue
        req = (v.get("requirements") or {}).get("level") or v.get("drop_level") or 1
        base_req.setdefault(v["name"], req)
    classes = json.load(open(os.path.join(out_dir, "poe1_bases.json"), encoding="utf-8"))

    uniques = []
    seen = set()
    for u in parse_uniques(pob):
        cls = classes.get(u["base"])
        if not cls or (u["name"], u["base"]) in seen:
            continue
        seen.add((u["name"], u["base"]))
        level = max(u["level"], base_req.get(u["base"], 1))
        text = " | ".join(u["mods"]).lower()
        useful = [m.lower() for m in u["mods"] if not NOT_OFFENCE.search(m.lower())]
        stats = [sid for sid, _, pats in STATS if any(re.search(p, m) for p in pats for m in useful)]
        if u.get("linked"):
            stats.append("links")
        uniques.append({"name": u["name"], "base": u["base"], "class": cls, "level": level, "mods": u["mods"], "stats": stats})
    uniques.sort(key=lambda x: (x["level"], x["name"]))

    gems = {}
    gtext = open(os.path.join(pob, "src/Data/Gems.lua"), encoding="utf-8").read()
    for m in re.finditer(r'name = "([^"]+)",.*?tags = \{(.*?)\},', gtext, re.S):
        name, tags = m.group(1), re.findall(r"(\w+) = true", m.group(2))
        if "grants_active_skill" not in tags or "support" in tags:
            continue
        stats = []
        for t in tags:
            for s in GEM_TAG_STATS.get(t, []):
                if s not in stats:
                    stats.append(s)
        gems.setdefault(name, {"tags": [t for t in tags if t not in ("grants_active_skill", "intelligence", "strength", "dexterity")], "stats": stats})

    # Gem colours (supports too), keyed by the game's gem id and by name: r / g / b / w.
    colours = {}
    for m in re.finditer(r'name = "([^"]+)",.*?gameId = "([^"]+)",.*?tags = \{(.*?)\},', gtext, re.S):
        name, gid, tags = m.group(1), m.group(2), re.findall(r"(\w+) = true", m.group(3))
        c = "r" if "strength" in tags else "g" if "dexterity" in tags else "b" if "intelligence" in tags else "w"
        colours.setdefault(gid, c)
        colours.setdefault(name, c)
        colours.setdefault(name.replace(" Support", ""), c)
    json.dump(colours, open(os.path.join(out_dir, "gem_colours.json"), "w"), separators=(",", ":"))

    json.dump(uniques, open(os.path.join(out_dir, "poe1_uniques.json"), "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    json.dump(gems, open(os.path.join(out_dir, "poe1_gems.json"), "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    json.dump([{"id": s, "name": n} for s, n, _ in STATS], open(os.path.join(out_dir, "levelling_stats.json"), "w"), separators=(",", ":"))
    print(len(uniques), "uniques,", len(gems), "gems")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else "core/data")
