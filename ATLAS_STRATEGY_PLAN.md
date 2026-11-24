# Atlas Strategy Builder – Plan

A new feature for the existing Atlas planner in the PoE filter tool. The user picks the strategies they want to run (e.g. Ritual + Harvest) and the tool gives them an Atlas tree they can refine.

## Already built (20–21 Nov)

Atlas strategies page and F7 window (see HANDOFF.md "Done"): pasting a maxroll farming strategy link or a pathofexile.com atlas tree link imports and shows the tree (`core/src/atlas.rs`, `src/Atlas.tsx`, `src/AtlasTreeView.tsx`, tree data `public/atlas_poe1.json`). Creator picks should reuse that import, not add a second one.

## Goal

Get a good Atlas tree for the chosen strategies, either from a creator's proven tree or built by the tool. It plugs into the existing Atlas planner, which can already import a tree.

## Mode 1: Creator Picks

- Pick one or more strategies (e.g. Ritual, Harvest).
- Show matching trees from creators. Sources: Maxroll, Mobalytics, and YouTube creators who link trees in guides or video descriptions.
- Each card shows: creator, strategy, league/patch, last updated.
- Click a card to load the tree into the Atlas planner, then adjust it.
- Save the edited copy under its own name; the original stays untouched (same idea as filter versions).

### Open questions

- Where creator trees come from: some sites may have readable links, others may need a hand-kept list. Check each site's terms.
- Keeping the list fresh each league and hiding trees from old patches.

## Mode 2: Optimiser

- Pick strategies and a priority: most profit, fewest points, or balanced.
- Suggest a tree that spends points on the nodes that matter most for those strategies, and flag nodes that do little.
- Show points spent, points left, and which nodes are doing the most work.
- Lock nodes you always want and block nodes you never want; the tool fills in the rest.

### The hard part

The optimiser needs a rough value for each node per strategy. Options, simplest first:

1. Hand-made list of key nodes per strategy.
2. Learn from creator trees: which nodes most creators take for a strategy.
3. Later: real results from the wealth tracker (chaos/divine per hour per saved strategy) to see which trees actually earn more.

## Links to Other Planned Features

- Wealth tracker: save each tree under a strategy name so earnings can be compared per tree.
- Map check: flag map mods that clash with the chosen strategy.

## Progress

- 23 Nov: step 1 built ("Build an atlas from strategies" on the Atlas page, layout C). 19 maxroll trees for 3.30 in
  `public/atlas_picks.json` (`node tools/build_atlas_picks.mjs` to refresh). Mobalytics and YouTube not added yet.
- Scarabs are gone since 3.30: mechanics go in the map device as Threads. Tiles use thread pictures.
- Saving a pick already makes a separate copy (the creator's tree stays on maxroll), so step 2 is about editing
  the copy's tree in the app, which isn't possible yet.

- 23 Nov: step 3 built ("Build my own" tab, layout A): optimiser in `core/src/atlas_opt.rs` using options 1 + 2 below
  (node text / tree areas, and which nodes creator trees take). Option 3 (wealth tracker results) is still to do.

- 24 Nov: option 3 started: map runs and earnings sessions now remember the tree in use, and each saved tree shows
  what it earned (`atlas::tree_results`). Feeding those results back into the optimiser's node values is still to do.

## Suggested Build Order

1. Creator picks with a small hand-picked list of trees, loaded into the existing planner.
2. Saving edited copies as separate versions.
3. Basic optimiser using a per-strategy node list, with lock and block.
4. Use creator trees and wealth tracker results to make the optimiser smarter.

## To Check in Game

- Imported creator trees match what the creator shows.
- Loading and editing a tree never changes the saved original.
