import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ItemPanel from "./ItemPanel";
import { CustomStyle, GAME_COLOURS } from "./StyleEditor";
import { Item } from "./types";

type Found = { base_type: string; item_class: string };
type Preset = { id: string; title: string; hint: string; count: number };

const GEAR = ["Abyss Jewels", "Amulets", "Belts", "Body Armours", "Boots", "Bows", "Claws", "Daggers", "Gloves", "Helmets",
  "Jewels", "One Hand Axes", "One Hand Maces", "One Hand Swords", "Quivers", "Rings", "Rune Daggers", "Sceptres", "Shields",
  "Staves", "Thrusting One Hand Swords", "Two Hand Axes", "Two Hand Maces", "Two Hand Swords", "Wands", "Warstaves",
  "Life Flasks", "Mana Flasks", "Hybrid Flasks", "Utility Flasks", "Tinctures", "Maps"];
const WEAPONS = ["Bows", "Claws", "Daggers", "One Hand Axes", "One Hand Maces", "One Hand Swords", "Rune Daggers", "Sceptres",
  "Staves", "Thrusting One Hand Swords", "Two Hand Axes", "Two Hand Maces", "Two Hand Swords", "Wands", "Warstaves"];
const CHIPS = ["Purple", "Yellow", "Cyan", "Red", "Green", "Orange", "Pink", "White"];

/** The item a search result stands for (rarity can be changed in the panel). */
function toItem(f: Found): Item {
  const rarity = GEAR.includes(f.item_class) ? "Rare"
    : f.item_class === "Divination Cards" ? "Divination Card"
    : f.item_class.endsWith("Gems") ? "Gem" : "Currency";
  return { item_class: f.item_class, base_type: f.base_type, rarity, name: null, item_level: null, stack_size: null,
    quality: null, sockets: null, map_tier: null, corrupted: false, mirrored: false, unidentified: false };
}

/** A bold "stand out" look in a game colour: coloured text and border, beam, map icon. */
export function standOut(name: string): CustomStyle {
  const rgb = GAME_COLOURS[name].slice(1).match(/../g)!.map((h) => parseInt(h, 16)).concat(255);
  return { font_size: 42, text: rgb, border: rgb, background: [20, 20, 20, 240], beam: { colour: name, temp: false },
    minimap: { size: 1, colour: name, shape: "Star" }, sound: { kind: "Default" }, label_hidden: false };
}

function Chips({ value, onPick }: { value: string; onPick: (c: string) => void }) {
  return (
    <span className="se-chips">
      {CHIPS.map((c) => (
        <button key={c} title={c} className={value === c ? "se-chip on" : "se-chip"} style={{ background: GAME_COLOURS[c] }} onClick={() => onPick(c)} />
      ))}
    </span>
  );
}

export default function Find({ onChange }: { onChange: () => void }) {
  const [q, setQ] = useState("");
  const [results, setResults] = useState<Found[]>([]);
  const [chosen, setChosen] = useState<Found | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [weapon, setWeapon] = useState("Two Hand Maces");
  const [oldWeapon, setOldWeapon] = useState("");
  const [buildColour, setBuildColour] = useState("Cyan");
  const [farmColour, setFarmColour] = useState<Record<string, string>>({});
  const [msg, setMsg] = useState("");
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => { invoke<Preset[]>("list_farm_presets").then(setPresets).catch(() => {}); }, []);
  useEffect(() => {
    window.clearTimeout(timer.current);
    if (!q.trim()) { setResults([]); return; }
    timer.current = window.setTimeout(() => invoke<Found[]>("search_items", { q }).then(setResults).catch(() => setResults([])), 150);
  }, [q]);

  const done = (m: string) => { setMsg(m); onChange(); };
  const applyBuild = async () => {
    try {
      const item = toItem({ item_class: weapon, base_type: "" });
      await invoke("apply_custom_style", { item, style: standOut(buildColour), opts: { same_rarity: false, max_item_level: null } });
      if (oldWeapon) {
        for (const rarity of ["Normal", "Magic"]) {
          await invoke("apply_quick_action", { item: { ...toItem({ item_class: oldWeapon, base_type: "" }), rarity }, action: "hide", colour: null,
            opts: { same_rarity: true, max_item_level: null } });
        }
      }
      done(`Every ${weapon} now stands out in ${buildColour.toLowerCase()}${oldWeapon ? `, and normal/magic ${oldWeapon} are hidden` : ""}. Press F4 in game.`);
    } catch (e) { setMsg(String(e)); }
  };
  const applyFarm = async (p: Preset) => {
    const c = farmColour[p.id] ?? "Orange";
    try {
      await invoke("apply_farm_preset", { id: p.id, style: standOut(c) });
      done(`${p.title} items now stand out in ${c.toLowerCase()}. Remove it any time in My edits. Press F4 in game.`);
    } catch (e) { setMsg(String(e)); }
  };

  return (
    <>
      <div className="card">
        <h2>Find an item or item type</h2>
        <input className="text" autoFocus placeholder="Try “2h mace”, “ammy”, “chaos orb”, “div card”…" value={q}
          onChange={(e) => { setQ(e.target.value); setChosen(null); }} />
        {results.length > 0 && !chosen && (
          <ul className="results">
            {results.map((r) => (
              <li key={r.item_class + r.base_type} onClick={() => setChosen(r)}>
                {r.base_type ? <><span>{r.base_type}</span><span className="muted small">{r.item_class}</span></>
                  : <><span><b>Every {r.item_class}</b></span><span className="muted small">whole item type</span></>}
              </li>
            ))}
          </ul>
        )}
        {q.trim() && results.length === 0 && <div className="muted small">No matches.</div>}
        {chosen && (
          <div className="find-panel qm">
            <ItemPanel key={chosen.item_class + chosen.base_type} inGame={false}
              start={{ item: toItem(chosen), from_screen: false, screen_text: null, confidence: 1 }}
              onClose={() => setChosen(null)} onChanged={onChange} />
          </div>
        )}
      </div>

      <div className="card">
        <h2>Switching build?</h2>
        <div className="row">
          <span>Make every</span>
          <select value={weapon} onChange={(e) => setWeapon(e.target.value)}>{WEAPONS.map((w) => <option key={w}>{w}</option>)}</select>
          <span>stand out in</span>
          <Chips value={buildColour} onPick={setBuildColour} />
        </div>
        <div className="row">
          <span>and hide normal/magic</span>
          <select value={oldWeapon} onChange={(e) => setOldWeapon(e.target.value)}>
            <option value="">(nothing)</option>{WEAPONS.filter((w) => w !== weapon).map((w) => <option key={w}>{w}</option>)}
          </select>
          <span className="muted small">(your old weapon type)</span>
        </div>
        <button className="primary" onClick={applyBuild}>Apply</button>
      </div>

      <div className="card">
        <h2>Farming a mechanic?</h2>
        <p className="muted small">Make everything from it stand out. Each one is a single entry in My edits, so it's easy to remove later.</p>
        <div className="farm-grid">
          {presets.map((p) => (
            <div key={p.id} className="farm">
              <b>{p.title}</b>
              <span className="muted small">{p.hint} ({p.count})</span>
              <Chips value={farmColour[p.id] ?? "Orange"} onPick={(c) => setFarmColour({ ...farmColour, [p.id]: c })} />
              <button onClick={() => applyFarm(p)}>Make them stand out</button>
            </div>
          ))}
        </div>
      </div>
      {msg && <div className="save-bar"><span className="note">{msg}</span></div>}
    </>
  );
}
