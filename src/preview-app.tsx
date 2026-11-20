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
  tracked: [5, 6], profiles: [{ name: "Dump tabs", tabs: [0, 1, 4] }], active: "Dump tabs",
  snapshots: [snap(now - 7200, 160), snap(now - 3600, 141), snap(now - 60, 584)],
  missed: [["Blight Thread", 50, 133], ["Legion Thread", 18, 65.7], ["Bestiary Thread", 29, 47.8], ["Summon Carrion Golem of Hordes", 1, 21]]
    .map(([name, stack, value]) => ({ tab: "Frags", name, stack, value, x: 3, y: 2, listed: false, icon: "", places: 1 })),
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
    .map(([key, title, what]) => ({ key, title, what, tiers: [key], on: !extrasOff.has(key) })),
  tiers: Array.from({ length: 16 }, (_, i) => { const t = i + 1; const b = t >= 14 ? "a" : t >= 11 ? "b" : t >= 6 ? "c" : "d"; const tier = `maps_${b}_t${t}`;
    return { map_tier: t, band: b, tier, name: `Maps ${b} t${t}`, visibility: mapHidden.has(tier) ? "Hide" : "Show", style: mapStyle(b), bases: [], note: "", min_stack: null }; }),
  special: [["influenced", "Influenced maps (Shaper, Elder, Conquerors, Zana)", 4], ["blighted", "Blighted maps", 3], ["nightmare", "Valdo's, nightmare & guardian maps", 3]]
    .map(([k, title, n]) => ({ type_path: ["maps", k], title, group: "Maps", shared_bases: true,
      tiers: Array.from({ length: n as number }, (_, i) => ({ tier: "t" + i, name: "Tier " + i, visibility: "Show", style: mapStyle("a"), bases: [], note: "", min_stack: null })) })),
});
// ?build shows the F6 build window instead, with a sample build.
const buildWin = location.search.includes("build");
const gem = (name: string, colour: string, support = false) => ({ name, level: 20, quality: 20, enabled: true, support, colour });
const pob = async () => {
  const t = await fetch("/tree_poe1.json").then((r) => r.json());
  const ids = Object.keys(t.nodes).map(Number);
  const stage = (title: string, n: number) => ({ title, tree: { version: "3.30", nodes: ids.slice(200, 200 + n), url: "" },
    groups: [{ label: "Main", slot: "Body Armour", enabled: true, gems: [gem("Arc", "b"), gem("Spell Echo", "b", true), gem("Lightning Penetration", "b", true)] }],
    gear: [{ slot: "Helmet", rarity: "rare", name: "Doom Crown", base: "Hubris Circlet", lines: ["+80 to maximum Life"] }] });
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
  if (cmd === "get_icons") return {};
  if (cmd === "get_settings") return { key_atlas: "F7" };
  if (cmd === "atlas_list") return atlas;
  if (cmd === "atlas_use") { const a = args as { id: number; variant: number }; atlas = { ...atlas, active: a.id, variant: a.variant }; return atlas; }
  if (cmd === "atlas_rename") { const a = args as { id: number; name: string }; atlas = { ...atlas, list: atlas.list.map((s) => s.id === a.id ? { ...s, name: a.name } : s) }; return atlas; }
  if (cmd === "atlas_own_notes") { const a = args as { id: number; text: string }; atlas = { ...atlas, list: atlas.list.map((s) => s.id === a.id ? { ...s, own_notes: a.text } : s) }; return atlas; }
  if (cmd === "atlas_delete") { const a = args as { id: number }; atlas = { ...atlas, list: atlas.list.filter((s) => s.id !== a.id) }; return atlas; }
  if (cmd === "atlas_import") throw "The preview can't reach maxroll: try it in the app";
  if (cmd === "pob_get") return pob();
  if (cmd === "build_tree_opt") return [true, "P"];
  if (cmd === "get_tiers") return [];
  if (cmd === "get_map_tiers") return mapTiers();
  if (cmd === "set_tiers") { const a = args as { tiers: string[]; show: boolean | null; clearStyle?: boolean; reset?: boolean }; for (const t of a.tiers) { if (a.clearStyle) extrasOff.add(t); else if (a.reset) extrasOff.delete(t); } for (const t of a.tiers) { if (a.show === false) mapHidden.add(t); else if (a.show) mapHidden.delete(t); } return []; }
  return null;
}, { shouldMockEvents: true });

try { if (!localStorage.getItem("tab")) localStorage.setItem("tab", "wealth"); } catch { /* fine */ }
if (buildWin) document.body.classList.add("overlay");
if (atlasWindow) document.body.classList.add("overlay");
createRoot(document.getElementById("root")!).render(atlasWindow ? <div style={{ width: 430, height: 640 }}><AtlasOverlay /></div> : buildWin ? <BuildOverlay /> : <App />);
