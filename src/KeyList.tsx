import { HOTKEYS, HOTKEY_GROUPS, keyOf } from "./hotkeys";

/** The in-game keys, grouped by what they're for, each with a short name and how it works (hotkey card "C", 22 Nov 2026).
 *  Used by Home's "In game" card and Help. Keys he cleared in Settings are left out; `all` adds the notes key when he
 *  has set one. */
export default function KeyList({ keys, all = false }: { keys: Record<string, string>; all?: boolean }) {
  const groups = HOTKEY_GROUPS.map((g) => ({ ...g,
    list: HOTKEYS.filter((h) => h.group === g.id && keyOf(keys, h.setting) && (!h.extra || (all && h.setting === "key_notes"))) }))
    .filter((g) => g.list.length);
  return <>{groups.map((g) => (
    <div key={g.id} className="home-kgroup">
      <h4 aria-level={3}>{g.name}{g.note && <em>· {g.note}</em>}</h4>
      <div className="home-keys-grid">
        {g.list.map((h) => <div key={h.setting}><kbd>{keyOf(keys, h.setting)}</kbd><div><b>{h.name}</b><span>{h.how}</span></div></div>)}
      </div>
    </div>
  ))}</>;
}
