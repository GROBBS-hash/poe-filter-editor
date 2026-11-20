// Dev tool: the main window in a normal browser with sample data, to check layout and styling.
// With the editor running (`npm run tauri dev`), open http://localhost:1420/preview-app.html
// Pages that need more than the sample data show empty states; Wealth and the menu are the main targets.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import App from "./App";
import AtlasOverlay from "./AtlasOverlay";
import maxrollSample from "../core/data/test_maxroll_vg1ga30g.json";
import atlasData from "../public/atlas_poe1.json";
import BuildOverlay from "./BuildOverlay";
import "./styles.css";

const now = Math.floor(Date.now() / 1000);
const snap = (t: number, total: number) => ({ time: t, league: "Covens", total, divine: 217, tabs: [["Frags", total * 0.6], ["GEM", total * 0.2]], profile: "Dump tabs" });
const wealth = {
  account: "Exile#1234", has_sessid: true, league: "Covens", min: 10, minutes: 3, divine: 217, now,
  tabs: ["Frags", "BREACH", "DELI", "Div", "GEM", "Dump 1", "Dump 2"].map((name, index) => ({ index, name, kind: "PremiumStash", colour: "#c9a94a" })),
  tracked: [5, 6], profiles: [{ name: "Dump tabs", tabs: [0, 1, 4] }], active: "Dump tabs", rares: 12, rares_priced: 0, rares_todo: 12,
  snapshots: [snap(now - 7200, 160), snap(now - 3600, 141), snap(now - 60, 584)],
  missed: [["Blight Thread", 50, 133], ["Legion Thread", 18, 65.7], ["Bestiary Thread", 29, 47.8], ["Summon Carrion Golem of Hordes", 1, 21]]
    .map(([name, stack, value]) => ({ tab: "Frags", name, stack, value, x: 3, y: 2, listed: false, icon: "", places: 1 })),
  last_items: [["Frags", "Blight Thread", 30, 80], ["Frags", "Blight Thread", 20, 53], ["Frags", "Legion Thread", 18, 65.7], ["GEM", "Summon Carrion Golem of Hordes", 1, 21], ["GEM", "Vaal Grace", 1, 5], ["GEM", "Enlighten Support", 1, 160]]
    .map(([tab, name, stack, value]) => ({ tab, name, stack, value, x: 2, y: 4, listed: name === "Vaal Grace", icon: "", places: 1 })),
  sessions: [], current: { name: "Strand alch and go", league: "Covens", tabs: [5, 6], start: now - 2400, end: null, start_value: 300, last_value: 445, paused: 0, paused_at: null },
  strategies: [["Strand alch and go", 3, 2.5, 212]],
};

// Tiers → Map tiers: NeverSink-like map rules (whites T1-5, yellows T6-10, reds T11-16); hidden ones remembered here.
const mapHidden = new Set<string>(["maps_d_t1", "maps_d_t2"]);
const mapStyle = (b: string) => ({ font_size: 45, muted: false, text: b === "d" ? [255, 255, 255, 255] : [0, 0, 0, 255],
  background: b === "d" ? [20, 20, 0, 255] : b === "c" ? [235, 235, 120, 255] : [235, 235, 235, 255], border: b === "a" ? [255, 0, 0, 255] : null,
  beam: b === "d" ? "White" : "Yellow", minimap_parts: [b === "d" ? 2 : 1, b === "d" ? "White" : "Red", "Square"] });
const extrasOff = new Set<string>();
const mapTiers = () => ({
  extras: [["upgrade", "Upgrade borders", "A red border on maps above the tier you're running now, so upgrades stand out."], ["bands", "Tier group borders", "A dark border on red maps and a light one on lower maps."], ["implicit", "Implicit borders", "A gold border on maps with implicit mods (e.g. from the atlas)."]]
    .map(([key, title, what]) => ({ key, title, what, tiers: [key], on: !extrasOff.has(key), border: key === "upgrade" ? [220, 50, 0, 255] : key === "bands" ? [0, 0, 0, 255] : [255, 180, 0, 255], sample_tier: key === "implicit" ? 10 : 14 })),
  tiers: Array.from({ length: 16 }, (_, i) => { const t = i + 1; const b = t >= 14 ? "a" : t >= 11 ? "b" : t >= 6 ? "c" : "d"; const tier = `maps_${b}_t${t}`;
    return { map_tier: t, band: b, tier, name: `Maps ${b} t${t}`, visibility: mapHidden.has(tier) ? "Hide" : "Show", style: mapStyle(b), bases: [], note: "", min_stack: null }; }),
  special: [["influenced", "Influenced maps (Shaper, Elder, Conquerors, Zana)", 4], ["blighted", "Blighted maps", 3], ["nightmare", "Valdo's, nightmare & guardian maps", 3]]
    .map(([k, title, n]) => ({ type_path: ["maps", k], title, group: "Maps", shared_bases: true,
      tiers: Array.from({ length: n as number }, (_, i) => ({ tier: "t" + i, name: "Tier " + i, visibility: "Show", style: mapStyle("a"), bases: [], note: "", min_stack: null })) }))
    .concat([{ type_path: ["maps", "implicitmod"], title: "Citadel (conqueror) maps", group: "Maps", shared_bases: true, tiers: ["t1", "any"].map((tier) => ({ tier, name: tier === "t1" ? "Citadel maps, tier 14+" : "Citadel maps, lower tiers", visibility: "Show", style: { font_size: 45, muted: false, text: [145, 30, 220, 255], border: [145, 30, 220, 255], background: tier === "t1" ? [235, 220, 245, 255] : [200, 200, 200, 255], beam: "Purple", minimap_parts: [tier === "t1" ? 0 : 1, "Purple", "Square"] }, bases: [], note: "", min_stack: null })) }])
    .concat([{ type_path: ["maps", "influenced"], title: "Originator maps (Zana memories)", group: "Maps", shared_bases: true, tiers: [
      { tier: "zanamap", name: "Originator maps, tier 14+", visibility: "Show", style: { font_size: 45, muted: false, text: [100, 0, 122, 255], border: [100, 0, 122, 255], background: [255, 255, 255, 255], beam: "Red", minimap_parts: [0, "Red", "Square"] }, bases: [], note: "", min_stack: null },
      { tier: "zanamaplow", name: "Originator maps, lower tiers", visibility: "Show", style: { font_size: 45, muted: false, text: [145, 30, 220, 255], border: [145, 30, 220, 255], background: [200, 200, 200, 255], beam: "Purple", minimap_parts: [1, "Purple", "Square"] }, bases: [], note: "", min_stack: null }] }]),
});
// ?build shows the F6 build window instead, with a sample build.
const buildWin = location.search.includes("build");
const gem = (name: string, colour: string, support = false) => ({ name, level: 20, quality: 20, enabled: true, support, colour });
const pob = async () => {
  const t = await fetch("/tree_poe1.json").then((r) => r.json());
  const ids = Object.keys(t.nodes).map(Number);
  const stage = (title: string, n: number) => ({ title, tree: { version: "3.30", nodes: ids.slice(200, 200 + n), url: "" },
    groups: [{ label: "Main", slot: "Body Armour", enabled: true, gems: [gem("Arc", "b"), gem("Spell Echo", "b", true), gem("Lightning Penetration", "b", true)] }],
    gear: [{ slot: "Helmet", rarity: "rare", name: "Doom Crown", base: "Hubris Circlet", lines: ["+80 to maximum Life"], copy: "x" }, { slot: "Flask 1", rarity: "unique", name: "Coruscating Elixir", base: "Ruby Flask", lines: [], copy: "x" }, { slot: "Flask 2", rarity: "magic", name: "Examiner's Topaz Flask of the Rainbow", base: "Topaz Flask", lines: [] }],
    jewels: [{ node: 7960, notables: ["Doryani's Lesson", "Inspired Oppression", "Overshock"], item: { slot: "Jewel socket 7960", rarity: "rare", name: "8 Passive Only! Not 9!", base: "Large Cluster Jewel", lines: ["Adds 8 Passive Skills", "1 Added Passive Skill is Overshock"] } },
      { node: 26196, notables: [], item: { slot: "Jewel socket 26196", rarity: "unique", name: "Brutal Restraint", base: "Timeless Jewel", lines: ["Denoted service of 7000 dekhara in the akhara of Balbala"], copy: "x" } },
      { node: null, notables: [], item: { slot: "Belt Abyssal Socket 1", rarity: "rare", name: "Bleed + CB Avoidance Jewel", base: "Hypnotic Eye Jewel", lines: ["+12% chance to Avoid Bleeding"] } }] });
  return { build: { class: "Witch", ascendancy: "Elementalist", level: 50, active: 1, stages: [stage("Start of Maps", 90), stage("End of Early Maps", 120)] }, stage: 1, link: "", follow: false };
};
// Atlas strategies: maxroll's "Leaguestart Shrine/Magic Atlas" (variant 2 is a full tree) with sample notes.
// Add ?atlas-window to the address to see the F7 window instead of the main window.
const mxVariant = JSON.parse((maxrollSample as { data: string }).data).profiles[1].atlas.variants[0];
// Steps the way core/src/atlas.rs makes them (a step ends at a notable/keystone or where the next point isn't joined).
const atlasSteps = (pts: number[]) => {
  const nodes = atlasData.nodes as unknown as Record<string, [number, number, number, string]>;
  const adj = new Map<number, Set<number>>();
  for (const e of atlasData.edges as number[][]) { for (const [a, b] of [[e[0], e[1]], [e[1], e[0]]]) { if (!adj.has(a)) adj.set(a, new Set()); adj.get(a)!.add(b); } }
  const out: { name: string; node: number; nodes: number[]; total: number }[] = [];
  let cur: number[] = [], total = 0;
  pts.forEach((n, i) => {
    cur.push(n);
    const next = pts[i + 1];
    if (next === undefined || !adj.get(n)?.has(next) || [1, 2].includes(nodes[n]?.[2])) { total += cur.length; out.push({ name: nodes[n]?.[3] ?? "", node: n, nodes: cur, total }); cur = []; }
  });
  return out;
};
const note = (title: string, lines: [string, string][]) => ({ title, lines: lines.map(([kind, text]) => ({ kind, depth: 0, text })) });
let atlas = { active: 1, variant: 0, list: [
  { id: 1, name: "Leaguestart Shrine/Magic Atlas", site: "maxroll", link: "https://maxroll.gg/poe/farming-strategy-planner/vg1ga30g", author: "palsteron", imported: now,
    device: { maps: ["City Square Map"], threads: ["Ritual ×8", "Breach", "Incursion"], rolls: ["30% increased Pack size"], text: "Activate the Sacred Flora before doing ritual." },
    notes: [note("About", [["p", "Shrines and magic packs early in the league, cheap to run."]]), note("How to play", [["h", "Maps"], ["li", "Run any tier 16 with 30% pack size."], ["li", "Click every shrine."]])],
    variants: [{ name: "Shrines", nodes: [29045, ...mxVariant.history], steps: atlasSteps(mxVariant.history), choices: Object.entries(mxVariant.masteries).map(([n, e]) => [Number(n), e]),
      notes: [note("Map device & setup", [["li", "Breach Thread · Ritual Thread · Abyss Thread"]])] }],
    own_notes: "Sell shrine maps in bulk." },
  { id: 2, name: "My essence tree", site: "official", link: "", author: "", imported: now, device: null, notes: [], own_notes: "",
    variants: [{ name: "Atlas tree", nodes: [29045, ...mxVariant.history.slice(0, 40)], steps: atlasSteps(mxVariant.history.slice(0, 40)), choices: [], notes: [] }] },
] };
const atlasWindow = location.search.includes("atlas-window");
mockWindows(atlasWindow ? "atlas" : buildWin ? "build" : "main");
mockIPC((cmd, args) => {
  if (cmd === "overview") return { profiles: [{ id: "mid", name: "Mid-Game", game: "Poe1", base_file: "", output_file: "", rules: [] }], active: "mid", poe1_dir: "", poe2_dir: "", batch_on: false };
  if (cmd === "wealth_get") return wealth;
  if (cmd === "wealth_find") {
    const all = [
      { tab_index: 28, tab: "TENDY", name: "Woe Buckle Stygian Vise", kind: "Rare", stack: 1, x: 3, y: 1, icon: "", ilvl: 84, note: "", value: 210,
        mods: "Item Level: 84\n+32% to Chaos Resistance\n+47 to maximum Energy Shield\n38% increased Elemental Damage with Attack Skills" },
      { tab_index: 5, tab: "$$$", name: "Divine Orb", kind: "Currency", stack: 9, x: 0, y: 0, icon: "", ilvl: 0, note: "", value: 1953, mods: "Stack Size: 9/20" },
      { tab_index: 18, tab: "GEM", name: "Vaal Grace", kind: "Gem", stack: 1, x: 5, y: 0, icon: "", ilvl: 0, note: "in Rage Touch Paladin Gloves", value: null, mods: "Level: 20" },
    ];
    const q = String((args as { query: string }).query).toLowerCase().trim();
    const hits = q ? all.filter((f) => (f.name + f.kind + f.mods).toLowerCase().includes(q)) : [];
    return { items: 4231, time: now, league: "Covens", total: hits.length, hits };
  }
  if (cmd === "map_tool_get") return { source: "poe.re (preview)", bad: [1], wants: { quantity: null, pack_size: null, rarity: null, more_maps: null, more_currency: null, more_scarabs: null, more_cards: null, tier_min: null, kind: "any", corrupted_ok: true },
    mods: [{ id: 1, text: "Monsters reflect 18% of Elemental Damage", token: "s def", scary: 900, nightmare: false, trade_id: null, danger: "Elemental reflect" },
      { id: 2, text: "Players cannot Regenerate Life, Mana or Energy Shield", token: "t reg", scary: 800, nightmare: false, trade_id: null, danger: "No regen" }] };
  if (cmd === "map_tool_regex") return "\"!s def\"";
  // Real icons when the editor is running (vite serves the app's icon list), else none.
  if (cmd === "get_icons") return fetch("/preview-data/icons").then((r) => r.json()).catch(() => ({}));
  if (cmd === "get_settings") return { key_atlas: "F7" };
  if (cmd === "atlas_list") return atlas;
  if (cmd === "atlas_use") { const a = args as { id: number; variant: number }; atlas = { ...atlas, active: a.id, variant: a.variant }; return atlas; }
  if (cmd === "atlas_rename") { const a = args as { id: number; name: string }; atlas = { ...atlas, list: atlas.list.map((s) => s.id === a.id ? { ...s, name: a.name } : s) }; return atlas; }
  if (cmd === "atlas_own_notes") { const a = args as { id: number; text: string }; atlas = { ...atlas, list: atlas.list.map((s) => s.id === a.id ? { ...s, own_notes: a.text } : s) }; return atlas; }
  if (cmd === "atlas_delete") { const a = args as { id: number }; atlas = { ...atlas, list: atlas.list.filter((s) => s.id !== a.id) }; return atlas; }
  if (cmd === "atlas_import") throw "The preview can't reach maxroll: try it in the app";
  if (cmd === "pob_get") return pob();
  if (cmd === "build_tree_opt") return [true, "P"];
  if (cmd === "list_sounds") return [];
  // Tiers sections: public/_tiers.json when present (tierdump --json from a real filter), else none.
  if (cmd === "get_tiers") return fetch("/_tiers.json").then((r) => r.ok ? r.json() : []).catch(() => []);
  if (cmd === "get_map_tiers") return mapTiers();
  if (cmd === "set_tiers") { const a = args as { tiers: string[]; show: boolean | null; clearStyle?: boolean; reset?: boolean }; for (const t of a.tiers) { if (a.clearStyle) extrasOff.add(t); else if (a.reset) extrasOff.delete(t); } for (const t of a.tiers) { if (a.show === false) mapHidden.add(t); else if (a.show) mapHidden.delete(t); } return []; }
  return null;
}, { shouldMockEvents: true });

try { if (!localStorage.getItem("tab")) localStorage.setItem("tab", "wealth"); } catch { /* fine */ }
if (buildWin) document.body.classList.add("overlay");
if (atlasWindow) document.body.classList.add("overlay");
createRoot(document.getElementById("root")!).render(atlasWindow ? <AtlasOverlay /> : buildWin ? <BuildOverlay /> : <App />);
