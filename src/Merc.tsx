// Mercenary checker (28 Nov, look A + C picked by GROBBS): a small strip comes up by itself when a Mercenary talks to
// you (C); Ctrl+D on its items and skill gems fills it into the full "kill or recruit" window (A). The numbers come from
// `merc_app.rs` / core `merc.rs`: you can take ONE item when you beat it, so killing is worth its best item.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { WinBar, useFitHeight } from "./WinFrame";
import { ItemIcon } from "./icons";
import { worthText } from "./WorthLine";
import "./play.css";
import "./merc.css";

type Item = { name: string; base: string; rarity: string; chaos: number | null; how: string; listed: number | null };
type Gem = { name: string; support: boolean; tier: number | null };
type Support = { name: string | null; family: string | null; tier: number | null; brick: boolean };
type Skill = { name: string; supports: Support[]; brick: boolean };
type Verdict = { kind: "kill" | "recruit" | "check"; best: number | null; kill: number | null; recruit: number | null; why: string };
export type MercView = {
  name: string; stage: string; items: Item[]; gems: Gem[]; recruit: number | null; recruit_listed: number | null; recruit_how: string;
  helps: string[]; verdict: Verdict | null; divine: number | null; skills: Skill[];
};

const ROMAN = ["", "I", "II", "III"];

const STAGE: Record<string, string> = { met: "wants a duel", beaten: "beaten: your choice", joined: "recruited", took: "you took an item" };

function price(chaos: number | null, how: string, divine: number | null) {
  if (chaos != null) return worthText(chaos, "chaos", divine);
  return how === "pricing" ? "pricing…" : how === "error" ? "couldn't search" : "no price";
}

export function MercWindow() {
  const [v, setV] = useState<MercView | null>(null);
  useFitHeight(380, 60);
  useEffect(() => {
    const load = () => invoke<MercView>("merc_view").then(setV).catch(() => {});
    load();
    const un = listen<MercView>("merc-update", (e) => setV(e.payload));
    return () => { un.then((f) => f()); };
  }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") invoke("merc_hide"); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const hide = () => invoke("merc_hide");
  if (!v || !v.name) return (
    <div className="bo pw mc">
      <WinBar title="Mercenary check" hint="Esc hides" onClose={hide} compact />
      <div className="bo-body mc-empty">Comes up by itself when a Mercenary talks to you in a map.</div>
    </div>
  );
  const d = v.divine;
  const vd = v.verdict;
  const filled = v.items.length > 0 || v.gems.length > 0 || v.skills.length > 0;
  const unknown = v.skills.reduce((n, s) => n + s.supports.filter((p) => !p.name).length, 0);
  const skills = v.gems.filter((g) => !g.support);
  const supports = v.gems.filter((g) => g.support);
  const best = vd?.best != null ? v.items[vd.best] : null;
  const tag = vd?.kind === "kill" ? "Kill" : vd?.kind === "recruit" ? "Recruit" : "Check";
  return (
    <div className="bo pw mc">
      <WinBar title={<><span className="mc-sword" aria-hidden>⚔</span> {v.name}</>} hint={STAGE[v.stage] ?? ""} onClose={hide} compact>
        {!filled && vd && vd.kind !== "check" && <span className={`mc-tag ${vd.kind}`}>{tag}</span>}
      </WinBar>
      <div className="bo-body">
        {!filled ? (
          <div className="mc-hint">Hover each of its items and press <b>Ctrl+D</b>: they're priced here. Its skills are read off the screen when its window opens after the duel.</div>
        ) : (
          <>
            <div className="mc-verdict">
              <div className={`mc-box ${vd?.kind === "kill" ? "win kill" : ""}`}>
                <div className="mc-box-h">Kill: take {best ? best.name : "its best item"}</div>
                <div className="mc-big">{vd?.kill != null ? `≈ ${worthText(vd.kill, "chaos", d)}` : "—"}</div>
              </div>
              <div className={`mc-box ${vd?.kind === "recruit" ? "win recruit" : ""}`}>
                <div className="mc-box-h">Recruit: one like it on trade</div>
                <div className="mc-big">{v.recruit != null ? `≈ ${worthText(v.recruit, "chaos", d)}` : v.recruit_how === "pricing" ? "pricing…" : "—"}</div>
                {v.helps.map((h) => <div key={h} className="mc-help">{h}</div>)}
              </div>
            </div>
            {vd && <div className="mc-why">{vd.kind === "check" ? "" : `${tag}: `}{vd.why}</div>}
            {v.items.length > 0 && <div className="mc-sub">You can take one item. Best first:</div>}
            {v.items.map((it, i) => ({ it, i })).sort((a, b) => (b.it.chaos ?? -1) - (a.it.chaos ?? -1)).map(({ it, i }) => (
              <div key={i} className={`mc-row ${vd?.best === i ? "best" : ""}`}>
                <ItemIcon name={it.rarity === "Unique" ? it.name : null} base={it.base} size={26} />
                <span className="mc-name" title={it.base}>{it.name}{it.rarity !== "Unique" && it.name !== it.base ? <span className="muted small"> {it.base}</span> : null}</span>
                <span className="mc-price" title={it.listed != null ? `${it.listed} for sale` : it.how === "ninja" ? "poe.ninja" : undefined}>{price(it.chaos, it.how, d)}</span>
                <button className="link small" title="Open it in the price window" onClick={() => invoke("merc_details", { index: i })}>Details</button>
                <button className="link small" title="Not one of its items" onClick={() => invoke("merc_remove", { index: i, gem: false })}>✕</button>
              </div>
            ))}
            {v.skills.length > 0 && (
              <div className="mc-gems">
                <div className="mc-sub">Its skills{v.recruit_listed != null ? ` · ${v.recruit_listed} like it for sale` : ""}
                  {" "}<button className="link small" onClick={() => invoke("merc_recruit_details")}>Details</button></div>
                {v.skills.map((s) => (
                  <div key={s.name} className="mc-skill">
                    <div className={`mc-sk ${s.brick ? "bad" : ""}`}>
                      <ItemIcon base={s.name} size={18} />{s.name}{s.brick && <span className="mc-brick"> · bricks a Mercenary</span>}
                    </div>
                    {s.supports.map((p, i) => (
                      <span key={i} className={`mc-gem sup ${p.tier === 3 ? "t3" : ""} ${p.brick ? "bad" : ""} ${p.name ? "" : "unk"}`}
                        title={p.brick ? "Bricks a Mercenary" : p.name ? undefined : "Not known yet: hover it in game and press Ctrl+D"}>
                        {p.family && <ItemIcon name={`Increased ${p.family} Support`} base={`${p.family} Support`} size={16} />}
                        {p.name ?? "?"} {p.tier ? ROMAN[p.tier] : ""}
                      </span>
                    ))}
                  </div>
                ))}
                {unknown > 0 && <div className="mc-hint small mc-unk">{unknown === 1 ? "1 support isn't" : `${unknown} supports aren't`} known yet: hover it in game and press Ctrl+D (once each).</div>}
              </div>
            )}
            {v.skills.length === 0 && v.gems.length > 0 && (
              <div className="mc-gems">
                <div className="mc-sub">Its skills{v.recruit_listed != null ? ` · ${v.recruit_listed} like it for sale` : ""}
                  {" "}<button className="link small" onClick={() => invoke("merc_recruit_details")}>Details</button></div>
                {[...skills, ...supports].map((g) => {
                  const i = v.gems.indexOf(g);
                  return (
                    <span key={i} className={`mc-gem ${g.support ? "sup" : ""} ${g.tier === 3 ? "t3" : ""}`}>
                      <ItemIcon base={g.support ? `${g.name} Support` : g.name} size={16} />
                      {g.name}{g.support ? ` T${g.tier ?? "?"}` : ""}
                      <button className="link" title="Not its gem" onClick={() => invoke("merc_remove", { index: i, gem: true })}>✕</button>
                    </span>
                  );
                })}
              </div>
            )}
            {v.gems.length === 0 && v.skills.length === 0 && <div className="mc-hint small">Its skills are read when its window opens after the duel (or hover its skill list and press Ctrl+D).</div>}
          </>
        )}
      </div>
    </div>
  );
}
