import { useState } from "react";
import StyleEditor, { CurrentStyle, CustomStyle, fromCurrent } from "./StyleEditor";

export type RuleDetail = {
  label: string; show: boolean; item_class: string; base_type: string; rarity?: string | null;
  min_stack?: number | null; below_stack?: number | null; style: CurrentStyle;
};
export type RuleEdit = {
  show: boolean; style: CustomStyle | null; every_rarity: boolean; min_stack: number | null; below_stack: number | null;
};

const STACKY = ["Stackable Currency", "Gold", "Divination Cards", "Map Fragments", "Incubators",
  "Delve Stackable Socketable Currency", "Delve Socketable Currency", "Corpses", "Cursed Ducat"];

/** Change a saved edit: shown or hidden, which items it covers, and its look. */
export default function RuleEditor({ detail, onSave, onCancel }: {
  detail: RuleDetail; onSave: (e: RuleEdit) => void; onCancel: () => void;
}) {
  const [show, setShow] = useState(detail.show);
  const [every, setEvery] = useState(!detail.rarity);
  const [minStack, setMinStack] = useState<number | "">(detail.min_stack ?? "");
  const [below, setBelow] = useState<number | "">(detail.below_stack ?? "");
  const stacky = STACKY.includes(detail.item_class) || detail.min_stack != null || detail.below_stack != null;
  const gearRarity = detail.rarity && ["Normal", "Magic", "Rare", "Unique"].includes(detail.rarity);

  const edit = (style: CustomStyle | null): RuleEdit => ({
    show, style, every_rarity: every,
    min_stack: minStack === "" ? null : minStack,
    below_stack: below === "" ? null : below,
  });
  const num = (v: string) => (v === "" ? "" : Math.max(1, Math.floor(+v) || 1));

  return (
    <div className="re">
      <div className="re-row">
        <button className={show ? "re-tog on" : "re-tog"} onClick={() => setShow(true)}>Shown</button>
        <button className={!show ? "re-tog on hide" : "re-tog"} onClick={() => setShow(false)}>Hidden</button>
      </div>

      {gearRarity && (
        <label className="re-check">
          <input type="checkbox" checked={every} onChange={(e) => setEvery(e.target.checked)} />
          <span>Every rarity (not just {detail.rarity})</span>
        </label>
      )}

      {stacky && (
        <div className="re-row re-stack">
          <span>Stacks</span>
          <input type="number" min={1} placeholder="any" value={minStack}
            onChange={(e) => setMinStack(num(e.target.value))} />
          <span>or more, and under</span>
          <input type="number" min={2} placeholder="any" value={below}
            onChange={(e) => setBelow(num(e.target.value))} />
        </div>
      )}

      {show ? (
        <StyleEditor start={fromCurrent(detail.style, detail.rarity ?? "Currency")} label={detail.base_type}
          onSave={(style) => onSave(edit(style))} onCancel={onCancel} />
      ) : (
        <div className="se-actions">
          <button onClick={onCancel}>Cancel</button>
          <button className="primary" onClick={() => onSave(edit(null))}>Save</button>
        </div>
      )}
    </div>
  );
}
