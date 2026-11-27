import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** One slot you wear: what the search adds up (the trade site's weighted sum) and why. */
type Want = { stat: string; id: string; weight: number; why: string; on: boolean };
type Plan = { slot: string; current: string; wants: Want[]; now: number; not_counted?: string[] };

/** Stats you unticked stay unticked (per slot), so a capped resistance is switched off once. */
const OFF_KEY = "upgrades.off";
const loadOff = (): Record<string, string[]> => { try { return JSON.parse(localStorage.getItem(OFF_KEY) ?? "{}"); } catch { return {}; } };
const saveOff = (plans: Plan[]) => {
  try { localStorage.setItem(OFF_KEY, JSON.stringify(Object.fromEntries(plans.map((p) => [p.slot, p.wants.filter((w) => !w.on).map((w) => w.stat)])))); } catch { /* not kept */ }
};
type Found = { name: string; base: string; price: string; score: number; better_by: number; lines: string[]; icon: string };
type Upgrades = { total: number; url: string; items: Found[]; logged_in: boolean };

type From = "build" | "worn";
const FROM_KEY = "upgrades.from";

/** "Find something better" for each slot: cheapest items that beat what's there on what you need. Improves on the
 *  shown build's stage by default (it sits under the build), or on what your character wears. */
export default function UpgradeFinder({ stage }: { stage: number }) {
  const [from, setFromState] = useState<From>(() => (localStorage.getItem(FROM_KEY) as From) || "build");
  const [plans, setPlans] = useState<Plan[] | null>(null);
  const [who, setWho] = useState({ character: "", stage: "" });
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [res, setRes] = useState<Record<string, Upgrades | string>>({});
  const setFrom = (f: From) => { setFromState(f); try { localStorage.setItem(FROM_KEY, f); } catch { /* not kept */ } };

  useEffect(() => {
    const off = loadOff();
    setPlans(null); setRes({}); setOpen(null);
    invoke<{ plans: Plan[]; character: string; stage: string }>("upgrade_plans", { from, stage })
      .then((r) => {
        setWho({ character: r.character, stage: r.stage });
        setPlans(r.plans.map((p) => ({ ...p, wants: p.wants.map((w) => ({ ...w, on: !(off[p.slot] ?? []).includes(w.stat) })) })));
      })
      .catch(() => setPlans([]));
  }, [from, stage]);

  const picker = (
    <div className="uf-from">
      <span className="muted small">Improve on:</span>
      <button className={`chip ${from === "build" ? "on" : ""}`} onClick={() => setFrom("build")}>This build{who.stage ? ` (${who.stage})` : ""}</button>
      <button className={`chip ${from === "worn" ? "on" : ""}`} onClick={() => setFrom("worn")}>What {who.character || "my character"} wears</button>
    </div>
  );

  if (!plans) return <div className="uf">{picker}<div className="empty">Reading the gear…</div></div>;
  if (plans.length === 0) return (
    <div className="uf">{picker}<div className="empty">{from === "build"
      ? "This stage of the build has no armour or jewellery to improve on."
      : "I don't know what you're wearing yet. Log in on the Wealth page (or press Ctrl+D on an item and use \"I'm wearing this\"), then come back."}</div></div>
  );

  const toggle = (slot: string, stat: string) => {
    const next = plans.map((p) => p.slot !== slot ? p : { ...p, wants: p.wants.map((w) => w.stat === stat ? { ...w, on: !w.on } : w) });
    setPlans(next);
    saveOff(next);
  };
  const nowScore = (p: Plan) => {
    // The score of what you wear with only the switched-on stats; the backend scores it the same way.
    return p.wants.every((w) => w.on) ? p.now : null;
  };
  const find = async (p: Plan) => {
    setBusy(p.slot); setOpen(p.slot);
    try { setRes({ ...res, [p.slot]: await invoke<Upgrades>("upgrade_search", { plan: p, from, stage }) }); }
    catch (e) { setRes({ ...res, [p.slot]: String(e) }); }
    setBusy(null);
  };

  return (
    <div className="uf">
      {picker}
      <p className="muted small">Each search adds up the stats below (weighted) and only lists items that beat what you wear now, cheapest first, instant buyout.
        Untick a stat to leave it out. Weapons aren't here: compare those with Ctrl+D.</p>
      {plans.map((p) => {
        const r = res[p.slot];
        const score = nowScore(p);
        return (
          <div key={p.slot} className="uf-slot">
            <div className="uf-head">
              <span className="uf-name"><b>{p.slot}</b> <span className="muted">{p.current || "empty"}</span></span>
              {score != null && <span className="muted small">scores {Math.round(score)}</span>}
              <button className="primary small" disabled={busy != null} onClick={() => find(p)}>{busy === p.slot ? "Searching…" : "Find better"}</button>
            </div>
            <div className="uf-wants">
              {p.wants.map((w) => (
                <label key={w.stat} className={`uf-want ${w.on ? "on" : ""}`} title={w.why}>
                  <input type="checkbox" checked={w.on} onChange={() => toggle(p.slot, w.stat)} />
                  {w.stat}{w.weight !== 1 && <span className="muted"> ×{w.weight}</span>}
                </label>
              ))}
            </div>
            {open === p.slot && <div className="muted small uf-why">{p.wants.filter((w) => w.on).map((w) => w.why).join(" · ")}</div>}
            {(p.not_counted ?? []).length > 0 && (
              <div className="small uf-lose"><span className="muted">Yours also has (not counted, check you don't need it):</span> {p.not_counted!.join(" · ")}</div>
            )}
            {typeof r === "string" && <div className="uf-err">{r}</div>}
            {r && typeof r !== "string" && (
              !r.logged_in
                ? <div className="muted small">Opened on the trade site. Log in on the Wealth page to see the results here.</div>
                : <div className="uf-results">
                    <div className="muted small">{r.total.toLocaleString()} beat yours{r.items.length ? ". Cheapest:" : "."}{" "}
                      <button className="link small" onClick={() => invoke("open_url", { url: r.url })}>Open on the trade site</button></div>
                    {r.items.map((it, i) => (
                      <div key={i} className="uf-item">
                        {it.icon && <img src={it.icon} alt="" />}
                        <div className="uf-item-txt">
                          <div><b>{it.name}</b> <span className="muted small">{it.base !== it.name ? it.base : ""}</span></div>
                          <div className="small">{it.lines.slice(0, 6).join(" · ")}</div>
                        </div>
                        <div className="uf-item-side">
                          <div className="uf-price">{it.price}</div>
                          <div className="uf-better small">+{it.better_by}</div>
                        </div>
                      </div>
                    ))}
                  </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
