// Dev tool: the Ctrl+D price window in a normal browser, fed with saved price checks.
// Run `npm run dev` and open http://localhost:1420/preview.html
// The app saves each price check (and its trade result) to its data folder's "samples";
// vite.config.ts serves them here. With none saved yet, a built-in example is used.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { createRoot } from "react-dom/client";
import { useEffect, useState } from "react";
import TradeWindow from "./TradeWindow";
import "./styles.css";

type Json = any; // eslint-disable-line @typescript-eslint/no-explicit-any

const EXAMPLE: { check: Json; result: Json } = {
  check: {
    league: "Standard", from_ground: false, error: null, ninja: null, divine: 225, bulk: null, map: null,
    setup: {
      item: { item_class: "Rings", rarity: "Rare", name: "Wrath Coil", base_type: "Amethyst Ring", item_level: 70, corrupted: false, mirrored: false, unidentified: false },
      base_only: false, ilvl_min: null, links_min: null, gem_level: null, unknown: [], unknown_kinds: [], category: "accessory.ring",
      filters: [
        { id: "pseudo.pseudo_total_life", label: "+82 total maximum Life", value: 82, min: 82, enabled: true, kind: "pseudo", tier: null, roll: null },
        { id: "pseudo.pseudo_total_chaos_resistance", label: "+13% total to Chaos Resistance", value: 13, min: 13, enabled: false, kind: "pseudo", tier: null, roll: null },
        { id: "explicit.str", label: "+31 to Strength", value: 31, min: 31, enabled: false, kind: "explicit", tier: 5, roll: 0.55, range: "28–32" },
        { id: "explicit.acc", label: "11% increased Global Accuracy Rating", value: 11, min: 11, enabled: false, kind: "explicit", tier: 3, roll: 0.9, range: "6–11", junk: true },
        { id: "explicit.life", label: "+64 to maximum Life", value: 64, min: 64, enabled: false, kind: "explicit", tier: 1, roll: 0.2, range: "60–69" },
        { id: "explicit.dex", label: "+39 to Dexterity", value: 39, min: 39, enabled: false, kind: "explicit", tier: 3, roll: 0.1, range: "38–42" },
        { id: "pseudo.empty_prefix", label: "Open prefix (2 free)", value: 2, min: 1, enabled: false, kind: "pseudo", tier: null, roll: null },
      ],
    },
    compare: [{ slot: "Ring 1", current: "Early Finger Hole", rows: [{ stat: "Life", now: 50, this: 82 }, { stat: "Chaos res", now: 62, this: 13 }],
      gain: ["11% increased Global Accuracy Rating"], lose: ["28% increased Cold Damage"] }],
  },
  result: {
    total: 10000, url: "https://www.pathofexile.com/trade", divine: 225, summary: [0.04, 0.044],
    listings: Array.from({ length: 10 }, (_, i) => ({ amount: i < 6 ? 1 : 3, currency: i < 6 ? "alch" : "chaos", chaos: i < 6 ? 0.04 : 3,
      account: ["bluuflick", "VeenBearer", "Cerauful", "Ghorkus#7335"][i % 4], indexed: new Date(Date.now() - (i + 1) * 3.6e6 * 7).toISOString(),
      whisper: "@x Hi, I'd like to buy...", instant: i === 3, ilvl: 70 + i })),
  },
};

let current = EXAMPLE;
let mode: "saved" | "slow" | "error" = "saved";
let icons: Json = {};

mockWindows("trade");
mockIPC(async (cmd) => {
  if (cmd === "trade_search") {
    if (mode === "error") throw "Waiting on the trade site's limit: try again in 12s";
    await new Promise((r) => setTimeout(r, mode === "slow" ? 2500 : 400));
    if (!current.result) throw "No trade result saved for this item (search it in the app once)";
    return current.result;
  }
  if (cmd === "get_icons") return icons;
  if (cmd === "saved_list") return [];
  return null;
}, { shouldMockEvents: true });

function Controls() {
  const [list, setList] = useState<string[]>([]);
  const [pick, setPick] = useState("");
  const [m, setM] = useState(mode);
  const load = async (name: string) => {
    setPick(name);
    if (!name) current = EXAMPLE;
    else {
      const check = await fetch(`/preview-data/samples/${encodeURIComponent(name)}`).then((r) => r.json());
      const result = await fetch(`/preview-data/samples/${encodeURIComponent(name)}.result`).then((r) => r.json());
      current = { check, result };
    }
    emit("price-check", current.check);
  };
  useEffect(() => {
    fetch("/preview-data/icons").then((r) => r.json()).then((j) => { icons = j ?? {}; }).catch(() => {});
    fetch("/preview-data/samples").then((r) => r.json()).then((l) => { setList(l ?? []); load(""); }).catch(() => load(""));
  }, []);
  return (
    <div className="pv-controls">
      <b>Price window preview</b>
      <select value={pick} onChange={(e) => load(e.target.value)}>
        <option value="">Built-in example (rare ring)</option>
        {list.map((s) => <option key={s} value={s}>{s}</option>)}
      </select>
      <select value={m} onChange={(e) => { mode = e.target.value as typeof mode; setM(mode); }}>
        <option value="saved">Trade: saved result</option>
        <option value="slow">Trade: slow (2.5s)</option>
        <option value="error">Trade: rate-limit error</option>
      </select>
      <button onClick={() => load(pick)}>Press Ctrl+D again</button>
      <span className="muted small">{list.length} saved price checks</span>
    </div>
  );
}

const css = document.createElement("style");
css.textContent = `
  body.main { background: #0d0f13 radial-gradient(circle at 70% 30%, #2a2116, #0d0f13 70%); min-height: 100vh; }
  .pv-controls { display: flex; gap: 10px; align-items: center; padding: 10px 16px; color: #ddd; font: 13px system-ui; }
  .pv-frame { margin: 8px 16px; width: 380px; max-height: calc(100vh - 70px); overflow-y: auto; }
`;
document.head.appendChild(css);

createRoot(document.getElementById("root")!).render(
  <>
    <Controls />
    <div className="pv-frame"><TradeWindow /></div>
  </>,
);
