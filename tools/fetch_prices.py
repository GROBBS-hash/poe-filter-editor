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
                 "DeliriumOrb", "Omen", "Tattoo", "Runegraft", "AllflameEmber", "Artifact", "Astrolabe", "Ducat"]
POE1_STASH = ["UniqueWeapon", "UniqueArmour", "UniqueAccessory", "UniqueFlask", "UniqueJewel", "UniqueTincture",
              "UniqueRelic", "Incubator", "Invitation", "Memory", "Vial", "Corpse", "Wombgift"]
POE2_EXCHANGE = ["Currency", "Fragments", "Abyss", "UncutGems", "LineageSupportGems", "Essences", "SoulCores",
                 "Idols", "Runes", "Ritual", "Expedition", "Delirium", "Breach"]
POE2_STASH = ["UniqueWeapons", "UniqueArmours", "UniqueAccessories", "UniqueFlasks", "UniqueCharms", "UniqueJewels"]

debug = {}


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
    for it in core.get("items") or data.get("items") or []:
        if isinstance(it, dict) and it.get("id") is not None:
            names[str(it["id"])] = it.get("name") or it.get("text") or str(it["id"])
    out = []
    for line in data.get("lines") or []:
        value = num(line.get("primaryValue"))
        name = names.get(str(line.get("id"))) or line.get("name")
        if value is None or not name:
            continue
        out.append({"name": name, "base": name, "cat": typ, "value": value})
    return out, core


def stash_items(game, league, typ):
    data = get(f"/{game}/api/economy/stash/current/item/overview", league=league, type=typ)
    if not data:
        return []
    if f"{game}:stash" not in debug and data.get("lines"):
        debug[f"{game}:stash"] = {"line_sample": [{k: v for k, v in l.items() if k not in ("sparkLine", "lowConfidenceSparkLine", "explicitModifiers", "implicitModifiers", "flavourText", "tradeInfo")} for l in data["lines"][:2]]}
    out = []
    for line in data.get("lines") or []:
        links = line.get("links") or 0
        if links >= 5:  # 5/6-link prices would wildly overrate the base
            continue
        if line.get("corrupted") and typ.startswith("Unique"):
            continue
        value = num(line.get("chaosValue")) if game == "poe1" else num(line.get("primaryValue"))
        name = line.get("name")
        if value is None or not name:
            continue
        out.append({"name": name, "base": line.get("baseType") or name, "cat": typ, "value": value,
                    "variant": line.get("variant"), "listings": line.get("listingCount")})
    return out


def run(game, exchange_types, stash_types):
    leagues = get(f"/{game}/api/economy/leagues")
    if not leagues:
        print(f"{game}: no leagues", file=sys.stderr)
        return None
    league = leagues[0]
    league_id = league.get("id") or league.get("name")
    print(f"{game}: league {league_id}")
    items, unit, core_seen = [], "chaos" if game == "poe1" else "exalted", None
    for t in exchange_types:
        res = exchange(game, league_id, t)
        if res:
            got, core = res
            core_seen = core_seen or core
            items += got
            print(f"  {t}: {len(got)}")
    for t in stash_types:
        got = stash_items(game, league_id, t)
        items += got
        print(f"  {t}: {len(got)}")
    if core_seen and core_seen.get("primary"):
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
    with open(os.path.join(out_dir, "debug.json"), "w", encoding="utf-8") as f:
        json.dump(debug, f, indent=1, ensure_ascii=False, default=str)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "prices")
