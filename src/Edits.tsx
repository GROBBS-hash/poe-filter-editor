import { Tip } from "./Help";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { ProfileInfo, RuleInfo, RARITY_COLOUR } from "./types";
import ItemPanel, { RuleDetail, itemFromRule } from "./ItemPanel";
import { ItemIcon } from "./icons";
import DropPreview from "./DropPreview";
import { LookButton } from "./StyleEditor";
import { SavedLooksCard } from "./Looks";

const css = (c?: number[] | null) => (c ? `rgba(${c[0]},${c[1]},${c[2]},${(c[3] ?? 255) / 255})` : "transparent");

/** A tiny copy of how the rule looks in game. */
function Look({ r, onEdit }: { r: RuleInfo; onEdit: () => void }) {
  const d = r.detail;
  if (!d) return null;
  if (!d.show) return <span className="look-pill hidden">Hidden</span>;
  const st = d.style;
  if (st.label_hidden) return <span className="look-pill ghost">Icon & beam only</span>;
  return (
    <LookButton onClick={onEdit}><span className="look" style={{ color: css(st.text) || "#ccc", borderColor: css(st.border), background: css(st.background) }}>
      {d.base_type || d.item_class}
    </span></LookButton>
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
  const w = (d as { when?: string }).when;
  if (w === "levelling") parts.push("while levelling"); else if (w === "maps") parts.push("in maps");
  const st = d.style;
  if (d.show && !st.label_hidden) {
    if (st.beam) parts.push(`${st.beam} beam`);
    if (st.muted) parts.push("silent"); else if (st.sound) parts.push(st.sound);
  }
  return parts.join(" · ");
}

function PreviewCard({ profile }: { profile: ProfileInfo }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="card">
      <div className="row between">
        <h2>Preview drops</h2>
        <button className="link" onClick={() => setOpen(!open)}>{open ? "Hide" : "Show how drops look"}</button>
      </div>
      {open && <DropPreview profile={profile} />}
    </div>
  );
}

type Change = { index: number; at: number; what: string };

/** Recent changes to this filter, each with "go back to before this". */
function History({ profile, onChange }: { profile: ProfileInfo; onChange: () => void }) {
  const [list, setList] = useState<Change[]>([]);
  const [open, setOpen] = useState(false);
  const [msg, setMsg] = useState("");
  const load = () => invoke<Change[]>("get_history", { id: profile.id }).then(setList).catch(() => setList([]));
  useEffect(() => { load(); }, [profile]); // eslint-disable-line react-hooks/exhaustive-deps
  const back = async (c: Change) => {
    try {
      await invoke("go_back", { id: profile.id, index: c.index });
      setMsg(`Back to how it was before “${c.what}”. Changed your mind? Use the newest entry to undo that. Press F4 in game.`);
      onChange();
      load();
    } catch (e) { setMsg(String(e)); }
  };
  const when = (t: number) => {
    const d = new Date(t * 1000);
    const today = d.toDateString() === new Date().toDateString();
    return (today ? "" : d.toLocaleDateString() + " ") + d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };
  if (list.length === 0) return null;
  return (
    <div className="card">
      <div className="row between">
        <h2>History</h2>
        <button className="link" onClick={() => setOpen(!open)}>{open ? "Hide" : `Show ${list.length} recent change${list.length > 1 ? "s" : ""}`}</button>
      </div>
      {open && (
        <ul className="rules history">
          {list.map((c) => (
            <li key={c.index}>
              <span><span className="muted small">{when(c.at)}</span> {c.what}</span>
              <button className="link" onClick={() => back(c)}>Go back to before this</button>
            </li>
          ))}
        </ul>
      )}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}

export default function Edits({ profile, onChange, focus }: { profile: ProfileInfo; onChange: () => void; focus?: { label: string } | null }) {
  const [msg, setMsg] = useState("");
  const [open, setOpen] = useState<{ index: number; detail: RuleDetail } | null>(null);
  // Opened by clicking its look: start on the look editor.
  const [lookFirst, setLookFirst] = useState(false);
  const [q, setQ] = useState("");

  const remove = async (index: number) => {
    await invoke("delete_rule", { id: profile.id, index }).catch((e) => setMsg(String(e)));
    setOpen(null);
    onChange();
  };
  /** Off = kept in the list but left out of the filter (handy for trying a stricter setting and going back). */
  const setOff = async (r: RuleInfo, off: boolean) => {
    try {
      await invoke("set_rule_off", { id: profile.id, index: r.index, off });
      setMsg(`${off ? "Switched off" : "Switched back on"}: ${r.label.replace(/^Preset: [^:]+: /, "")}. Press F4 in game.`);
      onChange();
    } catch (e) { setMsg(String(e)); }
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

  // Opened from the quick menu's "Modify rule": open that edit.
  useEffect(() => {
    if (!focus) return;
    const r = [...profile.rules].reverse().find((x) => x.label === focus.label);
    if (!r) return;
    setQ("");
    edit(r.index).then(() => window.setTimeout(() =>
      document.querySelector(`[data-rule="${r.index}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" }), 80));
  }, [focus]); // eslint-disable-line react-hooks/exhaustive-deps

  // Group by item, newest first within each group; groups ordered by their newest edit.
  const needle = q.trim().toLowerCase();
  const groups = new Map<string, RuleInfo[]>();
  for (const r of [...profile.rules].reverse()) {
    if (needle && !`${r.label} ${r.detail?.item_class ?? ""}`.toLowerCase().includes(needle)) continue;
    // Presets group under their preset ("Levelling (Custom)"), everything else by item.
    const preset = r.label.match(/^Preset: ([^:]+):/);
    const key = r.detail ? `${r.detail.item_class}|${r.detail.base_type}` : preset ? `preset|${preset[1]}` : r.label;
    groups.set(key, [...(groups.get(key) ?? []), r]);
  }
  const unused = profile.rules.filter((r) => r.shadowed_by).length;

  return (
    <>
    <SavedLooksCard onChange={onChange} />
    <div className="card">
      <div className="row between">
        <h2>Edits in “{profile.name}” <Tip>Every change you've made to this filter. Newer edits win over older ones.
          Edit or Remove any of them; History lets you go back to before a change. Rebuild re-applies everything
          (you rarely need it: NeverSink updates do this automatically).</Tip></h2>
        <button onClick={rebuild} title="Puts all your edits back on top of the filter file. Rarely needed: NeverSink updates do this by themselves.">Rebuild filter</button>
      </div>
      <p className="muted small">Press <b>F4</b> in game to load {profile.output_file ? <>“{profile.output_file.replace(".filter", "")}”</> : "this filter"}.</p>
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
            const preset = !d && rules[0].label.match(/^Preset: ([^:]+):/);
            const title = d ? (d.base_type || `Every ${d.item_class}`) : preset ? `${preset[1]} preset` : rules[0].label;
            return (
              <div key={key} className="egroup">
                <div className="egroup-head">
                  {d?.base_type && <ItemIcon base={d.base_type} size={26} />}
                  <span style={{ color: d?.rarity ? RARITY_COLOUR[d.rarity] : undefined }}>{title}</span>
                  {d?.base_type && <span className="muted small">{d.item_class}</span>}
                </div>
                <ul className="rules">
                  {rules.map((r) => (
                    <li key={r.index} data-rule={r.index} className={`${open?.index === r.index ? "open" : ""} ${r.shadowed_by ? "unused" : ""} ${r.off ? "is-off" : ""}`}>
                      <div className="rule-line">
                        <button type="button" role="switch" aria-checked={!r.off} className={`onoff ${r.off ? "" : "on"}`}
                          title={r.off ? "Off: kept here but not in the filter. Click to switch it back on." : "On. Click to switch it off without deleting it."}
                          onClick={() => setOff(r, !r.off)} />
                        <span className="rule-main">
                          <Look r={r} onEdit={() => { setLookFirst(true); if (open?.index !== r.index) edit(r.index); }} />
                          <span className="rule-scope">{r.off ? "Off · not in the filter" : scope(r) || (r.detail ? "" : r.label.replace(/^Preset: [^:]+: /, ""))}</span>
                          {r.look && <span className="look-tag" title="Uses this saved look: change the look above and this edit changes too">{r.look}</span>}
                        </span>
                        <span>
                          <button className="link" onClick={() => { setLookFirst(false); edit(r.index); }}>{open?.index === r.index ? "Close" : "Edit"}</button>
                          <button className="link" onClick={() => remove(r.index)}>Remove</button>
                        </span>
                      </div>
                      {r.shadowed_by && <div className="unused-note">Never applies: “{r.shadowed_by}” is newer and always wins. Remove this one, or change the newer one.</div>}
                      {open?.index === r.index && (
                        <div className="rule-edit qm">
                          <ItemPanel key={r.index + (lookFirst ? "l" : "")} inGame={false} openLook={lookFirst}
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
    <PreviewCard profile={profile} />
    <History profile={profile} onChange={onChange} />
    </>
  );
}
