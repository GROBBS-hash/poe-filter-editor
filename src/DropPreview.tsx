import { friendly } from "./errors";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ProfileInfo, Item } from "./types";
import { GAME_COLOURS } from "./StyleEditor";
import { ItemIcon } from "./icons";

type Drop = {
  text: string; item: Item; visibility: string; reason: string; by_user_edit: boolean;
  style: { font_size?: number | null; text?: number[] | null; border?: number[] | null; background?: number[] | null;
    beam?: string | null; muted: boolean; sound?: string | null; minimap_parts?: [number, string, string] | null; label_hidden?: boolean };
};
const css = (c?: number[] | null, fb = "transparent") => (c ? `rgba(${c[0]},${c[1]},${c[2]},${(c[3] ?? 255) / 255})` : fb);

/** Sample drops (and items you've edited) shown exactly as the filter styles them. */
export default function DropPreview({ profile, theme }: { profile: ProfileInfo; theme?: string | null }) {
  const [area, setArea] = useState<number | null>(null); // null = your current area
  const [data, setData] = useState<{ drops: Drop[]; area_level: number } | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    setErr("");
    invoke<{ drops: Drop[]; area_level: number }>("preview_drops", {
      id: profile.id, theme: theme === undefined ? null : theme ?? "", areaLevel: area,
    }).then(setData).catch((e) => setErr(friendly(e)));
  }, [profile, theme, area]);

  if (err) return <div className="note">{err}</div>;
  if (!data) return <div className="muted small">Building preview…</div>;
  const shown = data.drops.filter((d) => d.visibility !== "Hide");
  const hidden = data.drops.filter((d) => d.visibility === "Hide");

  return (
    <div className="dp">
      <div className="row between">
        <span className="muted small">As drops would look in an area of level {data.area_level}{area === null ? " (where you are now)" : ""}. Hover a label to see why.</span>
        <span className="dp-area">
          {[[null, "Now"], [30, "Levelling"], [80, "Maps"]].map(([v, l]) => (
            <button key={String(l)} className={area === v ? "on" : ""} onClick={() => setArea(v as number | null)}>{l}</button>
          ))}
        </span>
      </div>
      <div className="dp-ground">
        {shown.map((d, i) => {
          const s = d.style;
          if (s.label_hidden) {
            return <span key={i} className="dp-ghost" title={`${d.text}: icon & beam only. ${d.reason}`}>
              {s.beam && <span className="dp-beam" style={{ background: GAME_COLOURS[s.beam] ?? "#fff" }} />}◆
            </span>;
          }
          const size = Math.max(11, Math.min(20, (s.font_size ?? 32) / 2.3));
          return (
            <span key={i} className={`dp-label ${d.by_user_edit ? "mine" : ""}`} title={d.reason}
              style={{ color: css(s.text, "#c8c8c8"), borderColor: css(s.border), background: css(s.background, "rgba(0,0,0,.75)"), fontSize: size }}>
              {s.beam && <span className="dp-beam" style={{ background: GAME_COLOURS[s.beam] ?? "#fff" }} />}
              <ItemIcon name={d.item.rarity === "Unique" ? d.item.name : null} base={d.item.base_type} size={Math.round(size * 1.3)} />
              {d.text}
              {!s.muted && s.sound && <span className="dp-snd" title={s.sound}>♪</span>}
            </span>
          );
        })}
      </div>
      {hidden.length > 0 && (
        <div className="dp-hidden">Hidden: {hidden.map((d, i) => <span key={i} title={d.reason}>{d.text}</span>)}</div>
      )}
      <div className="muted small">Coloured bar = beam · ♪ = drop sound · ◆ = icon & beam only (no label) · gold outline = one of your edits</div>
    </div>
  );
}
