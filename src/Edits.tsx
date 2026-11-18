import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { ProfileInfo, RuleInfo, RARITY_COLOUR } from "./types";
import ItemPanel, { RuleDetail, itemFromRule } from "./ItemPanel";

const css = (c?: number[] | null) => (c ? `rgba(${c[0]},${c[1]},${c[2]},${(c[3] ?? 255) / 255})` : "transparent");

/** A tiny copy of how the rule looks in game. */
function Look({ r }: { r: RuleInfo }) {
  const d = r.detail;
  if (!d) return null;
  if (!d.show) return <span className="look-pill hidden">Hidden</span>;
  const st = d.style;
  if (st.label_hidden) return <span className="look-pill ghost">Icon & beam only</span>;
  return (
    <span className="look" style={{ color: css(st.text) || "#ccc", borderColor: css(st.border), background: css(st.background) }}>
      {d.base_type || d.item_class}
    </span>
  );
}

/** What the rule covers, beyond the item: "stacks of 500+", "all rarities"... */
function scope(r: RuleInfo) {
  const d = r.detail;
  if (!d) return "";
  const parts: string[] = [];
  if (d.rarity && ["Normal", "Magic", "Rare", "Unique"].includes(d.rarity)) parts.push(d.rarity);
  if (!d.base_type) parts.push(`every ${d.item_class}`);
  if (d.min_stack && d.below_stack) parts.push(`stacks ${d.min_stack}–${d.below_stack - 1}`);
  else if (d.min_stack) parts.push(`stacks of ${d.min_stack}+`);
  else if (d.below_stack) parts.push(`stacks under ${d.below_stack}`);
  const st = d.style;
  if (d.show && !st.label_hidden) {
    if (st.beam) parts.push(`${st.beam} beam`);
    if (st.muted) parts.push("silent"); else if (st.sound) parts.push(st.sound);
  }
  return parts.join(" · ");
}

export default function Edits({ profile, onChange }: { profile: ProfileInfo; onChange: () => void }) {
  const [msg, setMsg] = useState("");
  const [open, setOpen] = useState<{ index: number; detail: RuleDetail } | null>(null);
  const [q, setQ] = useState("");

  const remove = async (index: number) => {
    await invoke("delete_rule", { id: profile.id, index }).catch((e) => setMsg(String(e)));
    setOpen(null);
    onChange();
  };
  const rebuild = async () => {
    try {
      const w = await invoke<string[]>("rebuild", { id: profile.id });
      setMsg(w.length ? w.join(" · ") : "Rebuilt from the latest base filter");
    } catch (e) { setMsg(String(e)); }
  };
  const edit = async (index: number) => {
    if (open?.index === index) return setOpen(null);
    try {
      const [, , detail] = await invoke<[string, number, RuleDetail]>("get_rule", { id: profile.id, index, label: null });
      setOpen({ index, detail });
    } catch (e) { setMsg(String(e)); }
  };

  // Group by item, newest first within each group; groups ordered by their newest edit.
  const needle = q.trim().toLowerCase();
  const groups = new Map<string, RuleInfo[]>();
  for (const r of [...profile.rules].reverse()) {
    if (needle && !`${r.label} ${r.detail?.item_class ?? ""}`.toLowerCase().includes(needle)) continue;
    const key = r.detail ? `${r.detail.item_class}|${r.detail.base_type}` : r.label;
    groups.set(key, [...(groups.get(key) ?? []), r]);
  }
  const unused = profile.rules.filter((r) => r.shadowed_by).length;

  return (
    <div className="card">
      <div className="row between">
        <h2>Edits in “{profile.name}”</h2>
        <button onClick={rebuild} title="Use after downloading a new filter file">Rebuild</button>
      </div>
      <p className="muted">In game, press <b>F4</b> to switch to and load “{profile.output_file.replace(".filter", "")}”. Newer edits win over older ones.</p>
      {unused > 0 && <div className="warn-line">{unused} edit{unused > 1 ? "s" : ""} never apply because a newer edit covers them (marked below).</div>}

      {profile.rules.length === 0 ? (
        <div className="empty">No edits yet. Hover an item in game and press F2.</div>
      ) : (
        <>
          {profile.rules.length > 5 && (
            <input className="text" placeholder="Search your edits (e.g. gold, chaos, belt)…" value={q} onChange={(e) => setQ(e.target.value)} />
          )}
          {groups.size === 0 && <div className="empty">Nothing matches “{q}”.</div>}
          {[...groups.entries()].map(([key, rules]) => {
            const d = rules[0].detail;
            const title = d ? (d.base_type || `Every ${d.item_class}`) : rules[0].label;
            return (
              <div key={key} className="egroup">
                <div className="egroup-head">
                  <span style={{ color: d?.rarity ? RARITY_COLOUR[d.rarity] : undefined }}>{title}</span>
                  {d?.base_type && <span className="muted small">{d.item_class}</span>}
                </div>
                <ul className="rules">
                  {rules.map((r) => (
                    <li key={r.index} className={`${open?.index === r.index ? "open" : ""} ${r.shadowed_by ? "unused" : ""}`}>
                      <div className="rule-line">
                        <span className="rule-main">
                          <Look r={r} />
                          <span className="rule-scope">{scope(r) || (r.detail ? "" : r.label)}</span>
                        </span>
                        <span>
                          <button className="link" onClick={() => edit(r.index)}>{open?.index === r.index ? "Close" : "Edit"}</button>
                          <button className="link" onClick={() => remove(r.index)}>Remove</button>
                        </span>
                      </div>
                      {r.shadowed_by && <div className="unused-note">Never applies: “{r.shadowed_by}” is newer and always wins. Remove this one, or change the newer one.</div>}
                      {open?.index === r.index && (
                        <div className="rule-edit qm">
                          <ItemPanel key={r.index} inGame={false}
                            start={{ item: itemFromRule(open.detail), from_screen: false, screen_text: null, confidence: 1 }}
                            replacing={{ id: profile.id, index: r.index, detail: open.detail }}
                            onClose={() => setOpen(null)} onChanged={onChange} />
                        </div>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            );
          })}
        </>
      )}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}
