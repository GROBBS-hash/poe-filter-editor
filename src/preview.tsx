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
import RELATED from "../core/data/related_items.json";

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
        { id: "explicit.str", label: "+31 to Strength", value: 31, min: 31, enabled: false, kind: "explicit", tier: 5, roll: 0.55, range: "28–32", affix: "suffix" },
        { id: "explicit.acc", label: "11% increased Global Accuracy Rating", value: 11, min: 11, enabled: false, kind: "explicit", tier: 3, roll: 0.9, range: "6–11", junk: true, affix: "suffix" },
        { id: "explicit.life", label: "+64 to maximum Life", value: 64, min: 64, enabled: false, kind: "explicit", tier: 1, roll: 0.2, range: "60–69", affix: "prefix" },
        { id: "explicit.dex", label: "+39 to Dexterity", value: 39, min: 39, enabled: false, kind: "explicit", tier: 3, roll: 0.1, range: "38–42", affix: "suffix" },
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
      whisper: "@x Hi, I'd like to buy...", instant: i === 3, ilvl: 70 + i,
      mods: [`+${60 + i * 2} to maximum Life`, `+${28 + i} to Strength`, i % 2 ? "+12% to Chaos Resistance" : "+40 to Dexterity"] })),
  },
};

let current = EXAMPLE;
let mode: "saved" | "slow" | "error" | "none" = "saved";
let icons: Json = {};
let prices: { name: string; value: number }[] = [];

// Same as core/src/related.rs, on the real price file, so the related-items panel shows real numbers here.
function related(name: string): Json {
  const price = (n: string) => { const v = prices.filter((i) => i.name === n).map((i) => i.value); return v.length ? Math.max(...v) : null; };
  const priced = (n: string, count = 1, me = false) => ({ name: n, value: price(n), count, me });
  const r: Json = { own: price(name), made_from: null, makes: null, group: [], set: false, drops: [] };
  const mf = RELATED.recipes.find((x) => x.item === name); if (mf) r.made_from = priced(mf.part, mf.count);
  const mk = RELATED.recipes.find((x) => x.part === name); if (mk) r.makes = priced(mk.item, mk.count);
  const g = RELATED.groups.find((x) => x.group.includes(name));
  if (g) {
    r.group = g.group.filter((n) => n !== r.made_from?.name && n !== r.makes?.name).map((n) => priced(n, 1, n === name));
    if (r.group.length === 1 && !g.set) r.group = [];
    r.set = g.set;
    r.drops = g.drops.map((n) => priced(n)).sort((a, b) => (b.value ?? -1) - (a.value ?? -1));
  }
  return r.made_from || r.makes || r.group.length || r.drops.length ? r : null;
}

// Fragments / invitations to try the related-items panel on.
const FRAGMENTS: Record<string, [string, string]> = {
  "Polaric Invitation": ["Map Fragments", "Normal"], "The Maven's Writ": ["Map Fragments", "Normal"],
  "Crescent Splinter": ["Map Fragments", "Normal"], "Fragment of the Chimera": ["Map Fragments", "Normal"],
  "Fragment of Knowledge": ["Map Fragments", "Normal"], "Al-Hezmin's Crest": ["Map Fragments", "Normal"],
  "Simulacrum": ["Map Fragments", "Normal"], "Screaming Invitation": ["Map Fragments", "Normal"],
};
function fragmentCheck(name: string): Json {
  const v = prices.filter((i) => i.name === name).map((i) => i.value);
  return { league: "Standard", from_ground: false, error: null, ninja: v.length ? [Math.max(...v), "chaos"] : null, divine: 225, bulk: name, map: null,
    setup: { item: { item_class: FRAGMENTS[name][0], rarity: FRAGMENTS[name][1], name: null, base_type: name, item_level: null, corrupted: false, mirrored: false, unidentified: false },
      base_only: true, ilvl_min: null, links_min: null, gem_level: null, unknown: [], unknown_kinds: [], category: null, filters: [] } };
}

mockWindows("trade");
mockIPC(async (cmd, args) => {
  if (cmd === "trade_search") {
    if (mode === "error") throw "Waiting on the trade site's limit: try again in 12s";
    await new Promise((r) => setTimeout(r, mode === "slow" ? 2500 : 400));
    if (mode === "none") return { listings: [], summary: null, total: 0, url: "", divine: null };
    if (!current.result) throw "No trade result saved for this item (search it in the app once)";
    return current.result;
  }
  if (cmd === "trade_untick_one") {
    // Shaped like his Entropy Grasp: which stat, left out, finds something.
    await new Promise((r) => setTimeout(r, 1200));
    const on = (((args as Json).setup as Json).filters as Json[]).filter((f) => f.enabled);
    return on.map((f, i) => ({ key: f.id, label: f.label, total: [16, 7, 2, 0, 0][i] ?? 0 })).sort((a, b) => b.total - a.total);
  }
  if (cmd === "trade_hidden") {
    // Shaped like core's trade::describe (the real one reads the search it would send).
    const s = (args as Json).setup as Json;
    const out = [s.buyout === false ? "Any listing" : "Instant buyout only"];
    if (s.item.rarity === "Unique" && s.item.name) out.push(`Named ${s.item.name}`);
    if (s.base_only || s.item.rarity === "Unique") out.push(`Base: ${s.base || s.item.base_type}`);
    if (s.item.rarity !== "Unique" && s.category) out.push("Not unique");
    if (!s.base_only && s.category && s.item.rarity !== "Unique") out.push(`Any ${String(s.category).split(".").pop()}`);
    if (s.ilvl_min) out.push(`Item level ${s.ilvl_min}+`);
    if (s.corrupted != null) out.push(s.corrupted ? "Corrupted" : "Not corrupted");
    return out;
  }
  if (cmd === "get_icons") return icons;
  if (cmd === "related_items") return related((args as Json).name);
  if (cmd === "trade_bulk") {
    // Offers shaped like his Stacked Deck check: two price fixers with tiny stock, the real market at 3c.
    const min = ((args as Json)?.minimum as number | null) ?? 0;
    // The Pack Leader, 25 Nov: 31 sellers, none sold.
    if ((args as Json)?.sold === 0) {
      const g = [1, 3, 3, 5, 5, 5, 5, 5, 9, 10, 10, 10, 10, 12, 15, 19, 20, 20, 20, 25].map((each, i) => ({ pay: each, pay_currency: "chaos", get: 1, get_currency: "x", each, stock: 1 + (i % 3), account: "Seller" + i, whisper: "@x hi" }));
      return { offers: g, url: "https://www.pathofexile.com/trade/exchange", divine: 177, asking: { each: 5, under: 0, sellers: 31 } };
    }
    // The Fletcher, 25 Nov: "6.63c" on poe.ninja, sold 1:1.
    if ((args as Json)?.card && ((args as Json)?.sold as number | null ?? 1e9) < 500) {
      const g = [1, 1, 1, 2, 3, 3, 3, 3, 5, 5, 5, 5, 5, 5, 5, 7, 9, 10, 10, 10].map((each, i) => ({ pay: each, pay_currency: "chaos", get: 1, get_currency: "x", each, stock: 1 + (i % 3), account: "Seller" + i, whisper: "@x hi" }));
      return { offers: g, url: "https://www.pathofexile.com/trade/exchange", divine: 177, asking: { each: 1, wait: 5, under: 8, sellers: 40 } };
    }
    if (((args as Json)?.sold as number | null ?? 1e9) < 50) {
      const g = [...Array(8).fill(1), 2, 3, 3, 3, 4, 4, 4, 5, 5, 5, 5, 5].map((each, i) => ({ pay: each, pay_currency: "chaos", get: 1, get_currency: "x", each, stock: 1 + (i % 3), account: "Seller" + i, whisper: "@x hi" }));
      return { offers: g, url: "https://www.pathofexile.com/trade/exchange", divine: 195, asking: { each: 5, under: 33, sellers: 80 } };
    }
    const o = (each: number, stock: number, account: string) => ({ pay: each, pay_currency: "chaos", get: 1, get_currency: "x", each, stock, account, whisper: "@x hi" });
    const offers = [o(1, 7, "Simi"), o(1, 3, "Klei"), o(2, 1, "Plasi"), o(3, 12, "Life"), o(3, 20, "Tat"), o(3, 41, "Nox"), o(3, 42, "Wu"), o(3, 3, "Necro")]
      .filter((x) => x.stock >= min);
    return { offers, url: "https://www.pathofexile.com/trade/exchange", divine: 217 };
  }
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
    else if (name.startsWith("frag:")) current = { check: fragmentCheck(name.slice(5)), result: null };
    else {
      const check = await fetch(`/preview-data/samples/${encodeURIComponent(name)}`).then((r) => r.json());
      const result = await fetch(`/preview-data/samples/${encodeURIComponent(name)}.result`).then((r) => r.json());
      current = { check, result };
    }
    emit("price-check", current.check);
  };
  // Dev hook: show any check from the browser console / a test script (e.g. one with a base_item).
  (window as unknown as { previewCheck: (c: unknown, r?: unknown) => void }).previewCheck = (c, r) => { current = { check: c as typeof current.check, result: (r ?? null) as Json }; emit("price-check", c); };
  useEffect(() => {
    // Prices and icons first, so the first window drawn already has them.
    Promise.all([
      fetch("/preview-data/prices").then((r) => r.json()).then((j) => { prices = j?.items ?? []; }).catch(() => {}),
      fetch("/preview-data/icons").then((r) => r.json()).then((j) => { icons = j ?? {}; }).catch(() => {}),
    ]).then(() => fetch("/preview-data/samples")).then((r) => r.json()).then((l) => { setList(l ?? []); load(""); }).catch(() => load(""));
  }, []);
  return (
    <div className="pv-controls">
      <b>Price window preview</b>
      <select value={pick} onChange={(e) => load(e.target.value)}>
        <option value="">Built-in example (rare ring)</option>
        {Object.keys(FRAGMENTS).map((f) => <option key={f} value={"frag:" + f}>Fragment: {f}</option>)}
        {list.map((s) => <option key={s} value={s}>{s}</option>)}
      </select>
      <select value={m} onChange={(e) => { mode = e.target.value as typeof mode; setM(mode); }}>
        <option value="saved">Trade: saved result</option>
        <option value="slow">Trade: slow (2.5s)</option>
        <option value="error">Trade: rate-limit error</option>
        <option value="none">Trade: nothing found</option>
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
  .pv-frame { margin: 8px 16px; width: max-content; max-width: calc(100vw - 32px); max-height: calc(100vh - 70px); overflow: auto; }
`;
document.head.appendChild(css);

createRoot(document.getElementById("root")!).render(
  <>
    <Controls />
    <div className="pv-frame"><TradeWindow /></div>
  </>,
);
