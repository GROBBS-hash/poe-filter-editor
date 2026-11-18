import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ItemPanel from "./ItemPanel";
import { ItemIcon } from "./icons";
import { CustomStyle, GAME_COLOURS } from "./StyleEditor";
import { Item } from "./types";

type Found = { base_type: string; item_class: string };
type Preset = { id: string; title: string; hint: string; count: number };
type Plan = {
  intent: "highlight" | "hide" | "icon" | "louder" | "quieter";
  target: Found; colour: string; rarity?: string | null; min_stack?: number | null; below_stack?: number | null; when?: string;
  summary: string; alternatives: Found[];
};

/** Does what an ask-box plan says, using the same building blocks as the buttons. */
async function carryOut(p: Plan) {
  const item = { ...toItem(p.target), ...(p.rarity ? { rarity: p.rarity } : {}) };
  const opts = { same_rarity: !!p.rarity, max_item_level: null, min_stack: p.min_stack ?? undefined, below_stack: p.below_stack ?? undefined, when: p.when ?? "always" };
  const base = standOut(p.colour);
  if (p.intent === "hide") return invoke("apply_quick_action", { item, action: "hide", colour: null, opts });
  const style: CustomStyle =
    p.intent === "icon" ? { ...base, label_hidden: true, sound: { kind: "Off" } }
    : p.intent === "louder" ? { ...base, font_size: 45, sound: { kind: "Builtin", id: "2", volume: 300 } }
    : p.intent === "quieter" ? { ...base, font_size: 26, text: [170, 170, 170, 255], border: null, beam: null, minimap: null, sound: { kind: "Off" } }
    : base;
  return invoke("apply_custom_style", { item, style, opts });
}

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

export default function Find({ onChange, view = "find" }: { onChange: () => void; view?: "find" | "presets" }) {
  const [q, setQ] = useState("");
  const [results, setResults] = useState<Found[]>([]);
  const [chosen, setChosen] = useState<Found | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [weapon, setWeapon] = useState("Two Hand Maces");
  const [oldWeapon, setOldWeapon] = useState("");
  const [buildColour, setBuildColour] = useState("Cyan");
  type LevelBuild = { id: string; title: string; hint: string; keep: string[] };
  const [levels, setLevels] = useState<LevelBuild[]>([]);
  const [allWeapons, setAllWeapons] = useState<string[]>([]);
  const [levelOn, setLevelOn] = useState<string | null>(null);
  type LevelOpts = { keep: string[]; rares_from: number; hide_armour: boolean };
  const [levelOpts, setLevelOpts] = useState<LevelOpts | null>(null);
  const [raresFrom, setRaresFrom] = useState(16);
  const [hideArmour, setHideArmour] = useState(false);
  const levelKeep = levelOpts?.keep ?? [];
  const [editing, setEditing] = useState<{ build: LevelBuild; keep: string[] } | null>(null);
  const [levelColour, setLevelColour] = useState("Cyan");
  const [farmColour, setFarmColour] = useState<Record<string, string>>({});
  const [msg, setMsg] = useState("");
  const [askText, setAskText] = useState("");
  const [asking, setAsking] = useState(false);
  const [planned, setPlanned] = useState<{ plan: Plan | null; used_ai: boolean } | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const loadLevels = () => invoke<{ builds: LevelBuild[]; all: string[]; current: string | null; options: LevelOpts | null }>("list_level_presets")
    .then((r) => {
      setLevels(r.builds); setAllWeapons(r.all); setLevelOn(r.current); setLevelOpts(r.options);
      if (r.options) { setRaresFrom(r.options.rares_from); setHideArmour(r.options.hide_armour); }
    }).catch(() => {});
  useEffect(() => { invoke<Preset[]>("list_farm_presets").then(setPresets).catch(() => {}); loadLevels(); }, []);
  useEffect(() => {
    window.clearTimeout(timer.current);
    if (!q.trim()) { setResults([]); return; }
    timer.current = window.setTimeout(() => invoke<Found[]>("search_items", { q }).then(setResults).catch(() => setResults([])), 150);
  }, [q]);

  const done = (m: string) => { setMsg(m); onChange(); };
  const askNow = async () => {
    if (!askText.trim()) return;
    setAsking(true);
    setPlanned(null);
    try { setPlanned(await invoke("ask", { text: askText })); } catch (e) { setMsg(String(e)); } finally { setAsking(false); }
  };
  const doPlan = async (p: Plan) => {
    try { await carryOut(p); setPlanned(null); setAskText(""); done(`Done: ${p.summary}. Press F4 in game.`); }
    catch (e) { setMsg(String(e)); }
  };
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
  const applyLevel = async (id: string, title: string, keep: string[] = []) => {
    try {
      await invoke("apply_level_preset", { id, options: { keep, rares_from: raresFrom, hide_armour: hideArmour && raresFrom > 0 }, style: standOut(levelColour) });
      setEditing(null);
      loadLevels();
      done(id === "off" ? "Levelling preset removed. Press F4 in game."
        : `${title}: while levelling, your weapons stand out in ${levelColour.toLowerCase()} and other weapons are hidden (uniques always show). Press F4 in game.`);
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
      {view === "find" && <>
      <div className="card">
        <h2>Just say what you want</h2>
        <div className="row">
          <input className="text grow" placeholder="e.g. show gold over 500 in a purple highlight · hide normal 2h swords · icon only for wisdom scrolls"
            value={askText} onChange={(e) => setAskText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && askNow()} />
          <button className="primary" onClick={askNow} disabled={asking || !askText.trim()}>{asking ? "Thinking…" : "Go"}</button>
        </div>
        {planned && (planned.plan ? (
          <div className="plan">
            <div>{planned.plan.summary}?</div>
            <div className="row">
              <button className="primary" onClick={() => doPlan(planned.plan!)}>Do it</button>
              <button onClick={() => setPlanned(null)}>Cancel</button>
              {planned.plan.alternatives.length > 0 && <span className="muted small">Not that? Did you mean:</span>}
              {planned.plan.alternatives.slice(0, 4).map((a) => (
                <button key={a.item_class + a.base_type} className="link"
                  onClick={() => setPlanned({ ...planned, plan: { ...planned.plan!, target: a, alternatives: [],
                    summary: planned.plan!.summary.replace(planned.plan!.target.base_type || `every ${planned.plan!.target.item_class}`, a.base_type || `every ${a.item_class}`) } })}>
                  {a.base_type || `every ${a.item_class}`}
                </button>
              ))}
            </div>
            {!planned.used_ai && <div className="muted small">Read without AI. Add a Claude API key in Settings for looser wording.</div>}
          </div>
        ) : <div className="note">Couldn't work out which item you mean. Try the item's name, like “Chaos Orb” or “two hand maces”.</div>)}
      </div>

      <div className="card">
        <h2>Find an item or item type</h2>
        <input className="text" autoFocus placeholder="Try “2h mace”, “ammy”, “chaos orb”, “div card”…" value={q}
          onChange={(e) => { setQ(e.target.value); setChosen(null); }} />
        {results.length > 0 && !chosen && (
          <ul className="results">
            {results.map((r) => (
              <li key={r.item_class + r.base_type} onClick={() => setChosen(r)}>
                {r.base_type ? <><span className="with-icon"><ItemIcon base={r.base_type} size={24} />{r.base_type}</span><span className="muted small">{r.item_class}</span></>
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

      </>}
      {view === "presets" && <>
      <div className="card">
        <div className="row between">
          <h2>Levelling build</h2>
          {levelOn && <button className="link" onClick={() => applyLevel("off", "")}>Turn off ({levelOn})</button>}
        </div>
        <p className="muted small">Like FilterBlade's levelling presets. Until maps (area level 68), the weapons for your build stand out
          and every other weapon type is hidden. Uniques and 4-links always show, and 3-links until area level 28 (like NeverSink). One at a time; maps aren't affected.
          Area level 16 is about the end of Act 2, 28 Act 4, 45 Act 8.</p>
        <div className="row"><span>Stand out in</span><Chips value={levelColour} onPick={setLevelColour} /></div>
        <div className="row">
          <span>White & blue weapons:</span>
          <select value={raresFrom} onChange={(e) => setRaresFrom(+e.target.value)}>
            <option value={0}>show the whole campaign</option>
            {[8, 12, 16, 24, 28, 45].map((n) => <option key={n} value={n}>until area level {n}{n === 16 ? " (NeverSink’s choice)" : ""}, then rares only</option>)}
          </select>
        </div>
        {raresFrom > 0 && (
          <label className="tick">
            <input type="checkbox" checked={hideArmour} onChange={(e) => setHideArmour(e.target.checked)} />
            Also hide white & blue armour (body, helmet, gloves, boots) from area level {raresFrom}
          </label>
        )}
        {levelOn && levelOpts && (levelOpts.rares_from !== raresFrom || levelOpts.hide_armour !== (hideArmour && raresFrom > 0)) && (
          <div className="muted small">Press your build's button to apply these changes.</div>
        )}
        <div className="farm-grid">
          {levels.map((p) => (
            <div key={p.id} className={`farm ${levelOn === p.title ? "on" : ""}`}>
              <b>{p.title}</b>
              <span className="muted small">{p.hint}</span>
              {levelOn === p.title && levelKeep.join() !== p.keep.join() && (
                <span className="small">Your pick: {levelKeep.join(", ")}</span>
              )}
              <div className="row">
                <button className={levelOn === p.title ? "" : "primary"} onClick={() => applyLevel(p.id, p.title, levelOn === p.title ? levelKeep : p.keep)}>
                  {levelOn === p.title ? "Update ✓" : "Use this"}</button>
                <button className="link" onClick={() => setEditing(editing?.build.id === p.id ? null
                  : { build: p, keep: levelOn === p.title ? levelKeep : p.keep })}>Edit weapons</button>
              </div>
            </div>
          ))}
        </div>
        {editing && (
          <div className="plan">
            <div><b>{editing.build.title}</b>: tick the weapon types you want while levelling. Unticked ones are hidden (uniques and 4-links still show).</div>
            <div className="ticks">
              {allWeapons.map((w) => (
                <label key={w} className="tick">
                  <input type="checkbox" checked={editing.keep.includes(w)}
                    onChange={(e) => setEditing({ ...editing, keep: e.target.checked ? [...editing.keep, w] : editing.keep.filter((x) => x !== w) })} />
                  {w}
                </label>
              ))}
            </div>
            <div className="row">
              <button className="primary" disabled={editing.keep.length === 0}
                onClick={() => applyLevel(editing.build.id, editing.build.title, editing.keep)}>Use with these</button>
              <button className="link" onClick={() => setEditing({ ...editing, keep: editing.build.keep })}>Reset to default</button>
              <button onClick={() => setEditing(null)}>Cancel</button>
            </div>
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
      </>}
      {msg && <div className="save-bar"><span className="note">{msg}</span></div>}
    </>
  );
}
