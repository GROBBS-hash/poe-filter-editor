import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { ItemIcon } from "./icons";
import { Tip } from "./Help";

type Setup = { character: [string, string, number] | null; stats: { id: string; name: string }[]; gems: Record<string, string[]>; league: string };
type Result = { unique: { name: string; base: string; class: string; level: number; mods: string[]; stats: string[] };
  slot: string; matched: string[]; wearable: boolean; trade: string; price: number | null };

const SLOTS = ["Weapon", "Off hand", "Helmet", "Body", "Gloves", "Boots", "Belt", "Ring", "Amulet", "Flask", "Jewel"];
// Always handy while levelling, whatever the skill.
const ALWAYS = ["movement_speed", "life", "resistances"];

const load = <T,>(k: string, d: T): T => { try { const v = localStorage.getItem(k); return v ? JSON.parse(v) : d; } catch { return d; } };
const save = (k: string, v: unknown) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch { /* not kept */ } };

/** Levelling gear helper: uniques you can wear now that suit your main skill, with trade links. */
export default function Gear() {
  const [setup, setSetup] = useState<Setup | null>(null);
  const [level, setLevel] = useState<number>(() => load("gear.level", 12));
  const [fromLog, setFromLog] = useState(false);
  const [ahead, setAhead] = useState<number>(() => load("gear.ahead", 0));
  const [gem, setGem] = useState<string>(() => load("gear.gem", ""));
  const [wanted, setWanted] = useState<string[]>(() => load("gear.wanted", ALWAYS));
  const [slots, setSlots] = useState<string[]>([]);
  const [types, setTypes] = useState<string[]>(() => load("gear.types", []));
  useEffect(() => { save("gear.types", types); }, [types]);
  const [results, setResults] = useState<Result[]>([]);
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => {
    invoke<Setup>("gear_setup").then((s) => {
      setSetup(s);
      if (s.character) { setLevel(s.character[2]); setFromLog(true); }
    }).catch(() => {});
  }, []);
  useEffect(() => { save("gear.level", level); save("gear.ahead", ahead); save("gear.gem", gem); save("gear.wanted", wanted); }, [level, ahead, gem, wanted]);
  useEffect(() => {
    const skill = (setup?.gems[gem] ?? []).filter((x) => wanted.includes(x));
    invoke<Result[]>("gear_picks", { level, ahead, wanted, skill, classes: [] }).then(setResults).catch(() => setResults([]));
  }, [level, ahead, wanted, gem, setup]);

  const gemNames = useMemo(() => Object.keys(setup?.gems ?? {}).sort(), [setup]);
  const pickGem = (g: string) => {
    setGem(g);
    const s = setup?.gems[g];
    if (s) setWanted([...new Set([...s, ...ALWAYS])]);
  };
  const toggle = (id: string) => setWanted(wanted.includes(id) ? wanted.filter((x) => x !== id) : [...wanted, id]);
  const statName = (id: string) => setup?.stats.find((s) => s.id === id)?.name ?? id;

  // Weapon types in the results (Wands, Staves, Sceptres…), for the type filter.
  const weaponTypes = [...new Set(results.filter((r) => r.slot === "Weapon" || r.slot === "Off hand").map((r) => r.unique.class))].sort();
  const shown = results.filter((r) => (slots.length === 0 || slots.includes(r.slot))
    && (types.length === 0 || !(r.slot === "Weapon" || r.slot === "Off hand") || types.includes(r.unique.class)));
  const groups = SLOTS.map((s) => [s, shown.filter((r) => r.slot === s)] as const).filter(([, l]) => l.length > 0);

  return (
    <>
      <div className="card">
        <h2>Your character and skill <Tip>Uniques you can wear at your level that help your main skill. Pick your skill gem and the
          matching stats are ticked for you (spells get cast speed and spell damage, minions get minion stats…); change the ticks
          to taste. Movement speed, life and resistances are always handy. <b>Trade</b> opens the official trade site for it,
          cheapest first. Your level comes from the game's log when you level up.</Tip></h2>
        <div className="row gear-top">
          <label>Level <input type="number" min={1} max={100} value={level}
            onChange={(e) => { setLevel(Math.max(1, Math.min(100, +e.target.value || 1))); setFromLog(false); }} /></label>
          {setup?.character && (
            <span className="muted small">{fromLog ? "from your game: " : "your game says: "}{setup.character[0]} ({setup.character[1]}) level {setup.character[2]}
              {!fromLog && <button className="link small" onClick={() => { setLevel(setup.character![2]); setFromLog(true); }}>use it</button>}</span>
          )}
          <label>Show <select value={ahead} onChange={(e) => setAhead(+e.target.value)}>
            <option value={0}>only what I can wear now</option>
            <option value={5}>+ up to 5 levels ahead</option>
            <option value={10}>+ up to 10 levels ahead</option>
          </select></label>
        </div>
        <div className="row">
          <label className="grow">Main skill
            <input className="text" list="gem-list" placeholder="e.g. Fireball, Raise Zombie, Split Arrow…" value={gem}
              onChange={(e) => (setup?.gems[e.target.value] ? pickGem(e.target.value) : setGem(e.target.value))} />
          </label>
          <datalist id="gem-list">{gemNames.map((g) => <option key={g} value={g} />)}</datalist>
        </div>
        <div className="gear-stats">
          {setup?.stats.map((s) => (
            <button key={s.id} className={`lvl-tile small ${wanted.includes(s.id) ? "on" : ""}`} onClick={() => toggle(s.id)}>{s.name}</button>
          ))}
          {wanted.length > 0 && <button className="link small" onClick={() => setWanted([])}>Clear (show everything)</button>}
        </div>
        <div className="gear-slots">
          <span className="muted small">Slots:</span>
          {SLOTS.map((s) => (
            <button key={s} className={`chip ${slots.includes(s) ? "on" : ""}`}
              onClick={() => setSlots(slots.includes(s) ? slots.filter((x) => x !== s) : [...slots, s])}>{s}</button>
          ))}
          {slots.length > 0 && <button className="link small" onClick={() => setSlots([])}>All slots</button>}
        </div>
        {weaponTypes.length > 1 && (
          <div className="gear-slots">
            <span className="muted small">Weapon types:</span>
            {weaponTypes.map((t) => (
              <button key={t} className={`chip ${types.includes(t) ? "on" : ""}`}
                onClick={() => setTypes(types.includes(t) ? types.filter((x) => x !== t) : [...types, t])}>{t}</button>
            ))}
            {types.length > 0 && <button className="link small" onClick={() => setTypes([])}>All types</button>}
          </div>
        )}
      </div>

      {groups.length === 0 && <div className="card empty">Nothing matches at level {level}. Untick a few stats or look further ahead.</div>}
      {groups.map(([slot, list]) => (
        <div key={slot} className="card">
          <h2>{slot} <span className="muted small">{list.length}</span></h2>
          <ul className="gear-list">
            {list.slice(0, 30).map((r) => {
              const k = r.unique.name + r.unique.base;
              return (
                <li key={k} className={r.wearable ? "" : "later"}>
                  <div className="gear-row" onClick={() => setOpen(open === k ? null : k)}>
                    <ItemIcon name={r.unique.name} base={r.unique.base} size={32} />
                    <span className="gear-name">
                      <b>{r.unique.name}</b>
                      <span className="muted small">{r.unique.base} · level {r.unique.level}{r.wearable ? "" : ` (in ${r.unique.level - level})`}</span>
                    </span>
                    <span className="gear-matched">{r.matched.map((m) => <span key={m} className="tag">{statName(m)}</span>)}</span>
                    <span className="gear-price">{r.price != null ? `${r.price < 10 ? r.price.toFixed(1) : Math.round(r.price)}c` : ""}</span>
                    <button onClick={(e) => { e.stopPropagation(); invoke("open_url", { url: r.trade }); }}>Trade</button>
                  </div>
                  {open === k && <ul className="gear-mods">{r.unique.mods.map((m, i) => <li key={i}>{m}</li>)}</ul>}
                </li>
              );
            })}
          </ul>
          {list.length > 30 && <div className="muted small">…and {list.length - 30} more. Tick fewer stats or pick a slot to narrow it down.</div>}
        </div>
      ))}
    </>
  );
}
