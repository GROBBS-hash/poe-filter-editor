# Atlas knowledge (3.30 Covens of Umbra, researched 25 Nov 2026)

What 20 researchers found about how atlas strategies work, boiled down for rebuilding the optimiser ("Build my own").
Full reports and data files: `.claude/atlas-research/` on GROBBS's PC (not committed: this repo is public).
Confidence: **[O]** official, **[D]** datamined, **[M]** measured, **[C]** creators agree / logged runs, **[T]** computed
from trees, **[I]** inference. Never hard-code an [I] without a switch. Prices move: read them live.

## 1. Rules of the game
- **The ten primaries never spawn on their own** [O]: Abyss, Bestiary, Betrayal, Blight, Breach, Expedition, Incursion, Legion,
  Ritual, Ultimatum need a Thread or Shaped Region. No Thread in the plan = every node of that mechanic is worth 0.
- **Threads**: 3 kinds x 8. Fixed bonus at 3 / 6 / 8, guaranteed random mods 1 / 2 / 3 at 1-3 / 4-7 / 8+ [O]. Use 1, 3, 6 or 8.
- **Three kinds of chance** [D/C]: *Surpassing* = linear, no cap, count = total/100 (essence, shrine, strongbox, exile, spirit, flora).
  *Plain +X%* stops at 100% (mercenary, ore, trials). *X% increased* multiplies a small hidden base (Mirage, Delirium, Evocator, Sentinel,
  Wildwood), and is worth 0 when the map already guarantees it (a map suffix / bought map does).
- **Five kinds of reward**; each is raised by different things:
  - **G ground loot**: kills x "more currency" x rarity. Rarity decides currency quality (map/monster rarity, not gear) [O].
  - **A Atlas Drops list** [O]: maps, threads, lifeforce, hiveblood, legion icons/splinters, essences per monster, blight chests,
    stackable ultimatum rewards, cluster jewels, boss exclusives.
  - **F fixed count** (betrayal safehouses, temples to sell, flora deck/thread drops): only count and speed.
  - **W reward window** (ritual favours, inscribed ultimatums): own nodes only.
  - **I boss items**: number of bosses. Flat chance per boss, not raised by pack/quantity (Milkybk, ~800 kills) [M].
- **Explicit effect** adds up: 8 smalls x2 + Invasive 25273 6 + Multiplying 30266 5 per Thread kind (max 15) + Mounting 34393 2 per map mod,
  + Malignant 28477 25 on one side. 6-mod map, 3 kinds = 49 (74). Each reward line is floored on its own [M] (pack 5 -> 6@20, 7@40, 8@60).
  It scales G and A, ~0 for F / W / I.
- **One of Eater / Exarch / Maven per map** [C]. Maven tree (Destructive Play 34384) -> every altar node 0. Conqueror / Shaper / Elder / Synthesis
  pack nodes are 0 outside those maps. Tier 1-15 nodes are 0 with four voidstones.
- **Divide and Conquer 19729**: Unstable (5397) kills 14402 27459 38684 40832(part) 10399 43387 14837 61993; Hives (22188) kills 3204 14287 21096
  2328 22689 28834 40852 41591.
- **"Per previous X" nodes** grow like n(n-1)/2 (19675 pits, 3204 breaches, 52197 rituals): buy the count first. 22270 (beasts) ~n².
- **Wording**: "Map Bosses" multiplies with every boss; "Final Map Boss" only with Destructive Play / Terror shrines.

## 2. How experts build
- Two families, never blended: **scaling tree** (ring + explicit effect + altars + one engine) and **rush / lottery tree** (cheap maps, speed,
  independent tickets). The middle is a dead zone.
- One engine first; then only the generic nodes it reads. Generic profile per engine (share of 8 effect / 21 rare / 15 pack / 13 rarity smalls):
  Legion 7.4/17/9/12, Breach 6.8/13/10/9, Harvest 3.9/11/8/5, Blight 4.4/10/8/7, Ritual 3.3/10/9/6, Boss 3.1/7/7/8, Essence 2.6/8/6/3,
  Ultimatum 3.2/2/8/2, Incursion 2.4/5/5/4, Mercs 2.3/2/5/4, Shrines 3.2/7/6/6, Ore 2.8/4/6/5 [T].
- Support engine = a 2-node head (Abyss under beasts: 57566 + 19675). Notables ~24, smalls ~125, 98-118 of them only connect.
- Bundles, all or nothing: 33008+41053; 48336+55003(+37197); 14203+20929; 20503+24679; 17982+2378; 61484+61620+64980; gateway pairs.
- Last 15-25 points: Lost in Transit 17982 + Yield Upon Yield 2378 + ore to ~92-100%.
- Never taken by anyone (mostly traps; 63753 and 5239 may just be untested): 875 4908 5239 9345 9856 10029 14402 20960 21453 30484 30545 32750 34389 37011 39180 39402 40046 40503 42908 52359 55213
  57913 60125 62654 63533 63753.
- Options: 28477 Prefix 49076; 27261 Players 62580 (many bosses: 15113); 712 Mirage 8618 (beasts: Delirium 17864; 0 if the map has it); 61698 Ore 9590;
  12887 own engine, else Shrine 20818; 28029 Acceleration 36379; price-driven: 61484 61620 64980 54685 41332 42092 56429 50917 12826 48364 7018 39977.

## 3. Travel nodes and routes
8 spokes (9 points to a hub; straight = maps, diagonal = threads), inner ring 2% maps, mid ring 3% threads, outside ring = 8 mod-effect nodes
with arcs: 0-45 pack, 45-90 rare, 90-135 magic, 135-180 rarity, 180-225 pack, 225-270 rare, 270-315 magic, 315-360 rarity.
Value per point, % of that goal's income [I, ranking firmer than decimals], full table in `travel_values.json`:

| | maps | threads | raw currency | altars | lifeforce | cards |
|---|---|---|---|---|---|---|
| 2% mod effect | 1.25 | 1.25 | 1.1 | .5 | .9 | .55 |
| 5% rares | .65 | .85 | 1.65 | .65 | .65 | .65 |
| 1% pack | .55 | .55 | .2 | .6 | .6 | .5 |
| 5% magic | .5 | .5 | .5 | .5 | .5 | .5 |
| 1% rarity | 0 | 0 | .15 | 0 | 0 | .15 |
| 2% maps | .75 | 0 | 0 | 0 | 0 | 0 |
| 3% threads | 0 | 1.0 | 0 | 0 | 0 | 0 |
| 5% influence pack | .9 | .9 | .6 | 2.2 | 0 | .8 |

Portals 0. Fixed-count goals: all ~0, take the shortest path. Free wins (same cost): 31171 via 31498 rare or 28873 pack; 50896 via 42882/14734;
3939 via 58767 not 13284; 50686 via 37420 not 33868; 29605 via 206 not 60645; 57123 via 63608 not 16780.
Mounting 34393 (4 pts) ≈ 4% a point; 43903 (5% maps per mod) = 15-22 map smalls; Seeing Double 39380 + Imperfect Reflections 16298 best raw package.

## 4. Pairs that multiply / clash
- **Multiply**: Abyss+Bestiary (revive, recapture); Legion+Abyss+Bestiary 8/8/8 (altars first, Legion last); Ritual+Rogue Exiles (36701, untested);
  Blight+Strongboxes (boxes reopen per lane reward); Essence+Exiles (58439); Flora+Essence (59781); Harvest+Verdant Lands 21235;
  Destructive Play+Mirage+Conqueror maps; Terror Shrines+shrine wheel+Map Boss altars; Drawn to Power 1928+Deep Bloodlines 59989+Haruspicy 7791;
  Anomie 13833+Amplified Artefacts 64462+Sinistral 61484; three cheap Threads+30266; 34384+64222+63753 (all want few monsters left).
- **Clash**: Delirium with anything stationary; Blight/Expedition open-map wants vs tight; Mirage copies no beasts, incursions, betrayal, wildwood;
  65225 (one Thread kind) vs 30266 / thread income; 17158 vs corrupting essences; 52315 vs rolled boxes; 62654 vs possession;
  9882 vs 38764; 13802 vs 41832; 48004 vs 60491; 57163 vs lifeforce; 34384 vs altars and vs rushing.
- **Pack size does nothing** for Legion, Blight chests, Betrayal, Essence, boss items.
- Incursion and Breach are each two farms (sell temples / kill; unstable / hives): ask which.

## 5. Goals (not only mechanics)
Raw currency, Threads, map drops, many bosses, boss rush, lifeforce, splinters, cards, fixed counts (essence, temples, safehouses).
- **Many bosses** A1 Terror Shrine maps (Exarch/Eater, 27261->15113, 18476, 9780, 58232, 20503, shrine smalls) ~15 bosses; A2 Destructive Play
  (Maven, no altars: 34384, 62212, 12302, 18476, Mirage) ~7-8.
- **Boss rush**: 64222 + 8 anomaly smalls (never 30545), map by fragment price, Mirage, speed. SnooBAE85 200 maps 47/h [M].
- **Map seller**: 43903, effect four, Players altars; market crowded, rank below the others.

## 6. Logged incomes (self-reported, strong builds; ceilings, never promises)
Fortress rush 82/h (90 maps, ~400 div build); Diviner orbs 53 (~120); rare stacking 47-62 gross (4 h); Abomination 47 (200); Terror Shrines 33 (48, at 40c
a map; asks are 119c now); Delirium 28 (48); Harvest 11-28 (40-50); conquerors 23 (40); Ritual 12-19 (40-100).
Maps per hour explains more than loot per map. Harvest 28 -> 56 div per 50 maps plain vs juiced [M].

## 7. Why our optimiser picks badly, and the fix order
Search is fine, values are wrong [T]. Held-out match with 22 creator trees: 0.54 now, 0.64 with only new values (creators agree ~0.50 with each other).
1. Tag by effect text, not by area label (33008->bosses, 27261->legion, 12887->abyss, 712->torment are wrong now).
2. Parsed effects (number, what it changes): `parse_nodes.cjs` / `parsed_nodes.json`, 93% + 7% yes/no.
3. Reward model per plan (unit x count x product), goals, Thread kinds, altar/Maven side, map (what it guarantees) as inputs.
4. Conflicts, dead sets, caps, bundles, traps; multiple-choice by live prices.
5. Learn weights from player trees (duplicates grouped); creators stay the untouched test.
6. Stable answers (HashSet order), re-solve with shaken weights -> "solid" / "depends on a guess" badges.
7. Calibrate on GROBBS's own `tree_results`. Show percentages, not chaos per hour.
Prototype: `.claude/atlas-research/data/proto.cjs` + `model.json` (placeholder weights).

## 8. Unknown, worth testing in game
Base chances; whether effect-added mods count for Mounting; bench crafts "2 additional Abysses" (3c) / "3 additional unstable Breaches" (4c);
Terror guards + chisels under Maven; Ritual + exiles; Breach sapling rush (8 cheap Breach Threads, no Breach nodes).

maxroll's robots.txt forbids data mining: the hand-kept pick list is as far as we go; no automatic collecting.
