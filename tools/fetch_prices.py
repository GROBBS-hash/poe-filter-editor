"""Fetches poe.ninja prices once for everyone (run hourly by GitHub Actions)
and writes small, simple files the app downloads:

    prices/poe1.json, prices/poe2.json
    {
      "game": "poe1", "league": "...", "updated": "2026-11-18T02:17:00Z",
      "unit": "chaos",
      "items": [{"name": "Divine Orb", "base": "Divine Orb", "cat": "Currency", "value": 180.0}, ...]
    }

poe.ninja asks desktop apps not to call its API from every user's PC, so
the app never talks to poe.ninja directly; it reads these files instead.
Standard library only.
"""

import json
import os
import sys
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone

UA = "poe-filter-editor price cache (github.com/GROBBS-hash/poe-filter-editor)"
BASE = "https://poe.ninja"

POE1_EXCHANGE = ["Currency", "Fragment", "DivinationCard", "Scarab", "Essence", "Fossil", "Resonator", "Oil",
                 "DeliriumOrb", "Omen", "Tattoo", "Runegraft", "AllflameEmber", "Artifact", "Astrolabe", "Ducat", "Thread"]
POE1_STASH = ["UniqueWeapon", "UniqueArmour", "UniqueAccessory", "UniqueFlask", "UniqueJewel", "UniqueTincture",
              "UniqueRelic", "Incubator", "Invitation", "Memory", "Vial", "Corpse", "Wombgift",
              "Map", "BlightedMap", "BlightRavagedMap", "Beast", "SkillGem"]
POE2_EXCHANGE = ["Currency", "Fragments", "Abyss", "UncutGems", "LineageSupportGems", "Essences", "SoulCores",
                 "Idols", "Runes", "Ritual", "Expedition", "Delirium", "Breach"]
POE2_STASH = ["UniqueWeapons", "UniqueArmours", "UniqueAccessories", "UniqueFlasks", "UniqueCharms", "UniqueJewels"]

debug = {}
# Item name -> picture URL, collected along the way (written to <game>_icons.json).
icons = {"poe1": {}, "poe2": {}}
CDN = "https://web.poecdn.com"


def add_icon(game, name, url):
    if name and url and name not in icons[game]:
        icons[game][name] = url if url.startswith("http") else CDN + url
poe2_exalted_per_divine = 1.0  # set from the exchange data before uniques are read


def get(path, **params):
    url = f"{BASE}{path}?{urllib.parse.urlencode(params)}" if params else f"{BASE}{path}"
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return json.load(r)
        except Exception as e:  # noqa: BLE001 - keep going, one bad category shouldn't stop the rest
            if attempt == 2:
                print(f"  ! {url}: {e}", file=sys.stderr)
                return None
            time.sleep(2 + attempt * 3)
        finally:
            time.sleep(0.6)  # be gentle


def num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return None


def trend(spark):
    """poe.ninja's last-7-days line: % change each day from a week ago, and the total change. Kept only when
    there's a line to draw (the app shows it in the price window)."""
    data = [num(x) for x in ((spark or {}).get("data") or [])]
    data = [x for x in data if x is not None]
    if len(data) < 2:
        return {}
    change = num((spark or {}).get("totalChange"))
    return {"trend": [round(x, 1) for x in data], "change": round(change if change is not None else data[-1], 1)}


def exchange(game, league, typ):
    """Exchange overview. Lines carry an id and a value in the 'primary'
    currency; names come from core.items. Converted to the game's unit."""
    data = get(f"/{game}/api/economy/exchange/current/overview", league=league, type=typ)
    if not data:
        return []
    core = data.get("core") or {}
    if typ in ("Currency",) and f"{game}:exchange" not in debug:
        debug[f"{game}:exchange"] = {
            "core_keys": list(core.keys()),
            "core_primary": core.get("primary"),
            "core_secondary": core.get("secondary"),
            "core_rates": core.get("rates"),
            "core_items_sample": (core.get("items") or [])[:3],
            "line_sample": (data.get("lines") or [])[:3],
            "top_keys": list(data.keys()),
        }
    names = {}
    for it in (data.get("items") or []) + (core.get("items") or []):
        if isinstance(it, dict) and it.get("id") is not None:
            names[str(it["id"])] = it.get("name") or it.get("text") or str(it["id"])
            add_icon(game, it.get("name"), it.get("image") or it.get("icon"))
    # PoE 2 prices come in Divine Orbs; convert to Exalted Orbs for finer steps.
    factor = 1.0
    if game == "poe2":
        factor = num((core.get("rates") or {}).get("exalted")) or 1.0
    out = []
    for line in data.get("lines") or []:
        value = num(line.get("primaryValue"))
        name = names.get(str(line.get("id"))) or line.get("name")
        if value is None or not name:
            continue
        # How much changed hands recently (value x number sold): tells a real price from a sale or two.
        vol = num(line.get("volumePrimaryValue"))
        extra = {"volume": round(vol * factor, 2)} if vol is not None else {}
        out.append({"name": name, "base": name, "cat": typ, "value": round(value * factor, 4), **extra, **trend(line.get("sparkline"))})
    return out, core


def stash_items(game, league, typ):
    data = get(f"/{game}/api/economy/stash/current/item/overview", league=league, type=typ)
    if not data:
        return []
    if f"{game}:stash" not in debug and data.get("lines"):
        debug[f"{game}:stash"] = {"line_sample": [{k: v for k, v in l.items() if k not in ("sparkLine", "lowConfidenceSparkLine", "explicitModifiers", "implicitModifiers", "flavourText", "tradeInfo")} for l in data["lines"][:2]]}
    out = []
    for line in data.get("lines") or []:
        add_icon(game, line.get("name"), line.get("icon"))
        if typ.startswith("Unique"):
            pass  # a unique's picture isn't its base's picture
        elif line.get("baseType"):
            add_icon(game, line.get("baseType"), line.get("icon"))
        links = line.get("links") or 0
        if links >= 5:  # 5/6-link prices would wildly overrate the base
            continue
        if line.get("corrupted") and typ.startswith("Unique"):
            continue
        if typ.startswith("Unique") and (line.get("listingCount") or 0) < 3:
            continue  # too few listings to trust (price fixing, typos)
        value = num(line.get("chaosValue")) if game == "poe1" else num(line.get("primaryValue"))
        if game == "poe2" and value is not None:
            value *= poe2_exalted_per_divine
        name = line.get("name")
        if value is None or not name:
            continue
        out.append({"name": name, "base": line.get("baseType") or name, "cat": typ, "value": round(value, 4),
                    "variant": line.get("variant"), "listings": line.get("listingCount"), **trend(line.get("sparkLine"))})
    if typ == "SkillGem":
        # Thousands of level/quality versions: keep each gem's cheapest (what a stash scan counts), so the file stays small.
        cheapest = {}
        for o in out:
            if o["name"] not in cheapest or o["value"] < cheapest[o["name"]]["value"]:
                cheapest[o["name"]] = {**o, "variant": None}
        out = list(cheapest.values())
    return out


def base_type_icons(game, league):
    """Pictures for every base type (poe.ninja's base type list); prices not used."""
    if game != "poe1":
        return
    data = get(f"/{game}/api/economy/stash/current/item/overview", league=league, type="BaseType")
    for line in (data or {}).get("lines") or []:
        add_icon(game, line.get("baseType") or line.get("name"), line.get("icon"))


def run(game, exchange_types, stash_types):
    leagues = get(f"/{game}/api/economy/leagues")
    if not leagues:
        print(f"{game}: no leagues", file=sys.stderr)
        return None
    league = leagues[0]
    league_id = league.get("id") or league.get("name")
    print(f"{game}: league {league_id}")
    items, unit, core_seen = [], "chaos" if game == "poe1" else "exalted", None
    global poe2_exalted_per_divine
    for t in exchange_types:
        res = exchange(game, league_id, t)
        if res:
            got, core = res
            core_seen = core_seen or core
            if game == "poe2" and num((core.get("rates") or {}).get("exalted")):
                poe2_exalted_per_divine = num(core["rates"]["exalted"])
            items += got
            print(f"  {t}: {len(got)}")
    for t in stash_types:
        got = stash_items(game, league_id, t)
        items += got
        print(f"  {t}: {len(got)}")
    base_type_icons(game, league_id)
    if game == "poe1" and core_seen and core_seen.get("primary"):
        unit = str(core_seen["primary"])
    return {
        "game": game,
        "league": league_id,
        "league_name": league.get("name"),
        "updated": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "unit": unit,
        "items": items,
    }


def main(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    for game, ex, st in (("poe1", POE1_EXCHANGE, POE1_STASH), ("poe2", POE2_EXCHANGE, POE2_STASH)):
        result = run(game, ex, st)
        if result and result["items"]:
            with open(os.path.join(out_dir, f"{game}.json"), "w", encoding="utf-8") as f:
                json.dump(result, f, separators=(",", ":"), ensure_ascii=False)
        if icons[game]:
            with open(os.path.join(out_dir, f"{game}_icons.json"), "w", encoding="utf-8") as f:
                json.dump(icons[game], f, separators=(",", ":"), ensure_ascii=False)
    with open(os.path.join(out_dir, "debug.json"), "w", encoding="utf-8") as f:
        json.dump(debug, f, indent=1, ensure_ascii=False, default=str)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "prices")
