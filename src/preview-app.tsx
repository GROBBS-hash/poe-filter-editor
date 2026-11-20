// Dev tool: the main window in a normal browser with sample data, to check layout and styling.
// With the editor running (`npm run tauri dev`), open http://localhost:1420/preview-app.html
// Pages that need more than the sample data show empty states; Wealth and the menu are the main targets.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import App from "./App";
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
const mapTiers = () => ({
  tiers: Array.from({ length: 16 }, (_, i) => { const t = i + 1; const b = t >= 14 ? "a" : t >= 11 ? "b" : t >= 6 ? "c" : "d"; const tier = `maps_${b}_t${t}`;
    return { map_tier: t, band: b, tier, name: `Maps ${b} t${t}`, visibility: mapHidden.has(tier) ? "Hide" : "Show", style: mapStyle(b), bases: [], note: "", min_stack: null }; }),
  special: [["influenced", "Influenced maps (Shaper, Elder, Conquerors, Zana)", 4], ["blighted", "Blighted maps", 3], ["nightmare", "Valdo's, nightmare & guardian maps", 3]]
    .map(([k, title, n]) => ({ type_path: ["maps", k], title, group: "Maps", shared_bases: true,
      tiers: Array.from({ length: n as number }, (_, i) => ({ tier: "t" + i, name: "Tier " + i, visibility: "Show", style: mapStyle("a"), bases: [], note: "", min_stack: null })) })),
});
mockWindows("main");
mockIPC((cmd, args) => {
  if (cmd === "overview") return { profiles: [{ id: "mid", name: "Mid-Game", game: "Poe1", base_file: "", output_file: "", rules: [] }], active: "mid", poe1_dir: "", poe2_dir: "", batch_on: false };
  if (cmd === "wealth_get") return wealth;
  if (cmd === "get_icons") return {};
  if (cmd === "get_tiers") return [];
  if (cmd === "get_map_tiers") return mapTiers();
  if (cmd === "set_tiers") { const a = args as { tiers: string[]; show: boolean | null }; for (const t of a.tiers) { if (a.show === false) mapHidden.add(t); else if (a.show) mapHidden.delete(t); } return []; }
  return null;
}, { shouldMockEvents: true });

try { if (!localStorage.getItem("tab")) localStorage.setItem("tab", "wealth"); } catch { /* fine */ }
createRoot(document.getElementById("root")!).render(<App />);
