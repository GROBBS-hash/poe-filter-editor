// "Jewels with my mods" (24 Nov, he chose C: both): one saved edit that highlights identified jewels with the mods
// he picked. The Tiers → Jewels card and F2 on a jewel both read and change that one edit.
import { invoke } from "@tauri-apps/api/core";
import type { CurrentStyle, CustomStyle } from "./StyleEditor";

export type JewelMod = { name: string; text: string; prefix: boolean; tags: string[] };
export type JewelRule = { mods: string[]; min: number; look: CurrentStyle | null; styled: boolean };

/** The edit's labels: "Highlight: …" until its look is changed, then "Style: …" (core rules.rs quick_rule). */
export const JM_LABELS = ["Highlight: every Jewels with my mods", "Style: every Jewels with my mods"];
const JM_ITEM = { item_class: "Jewels", rarity: "Magic", name: null, base_type: "", item_level: null, stack_size: null, quality: null,
  sockets: null, map_tier: null, corrupted: false, mirrored: false, unidentified: false };

let listCache: Promise<JewelMod[]> | null = null;
/** Every regular jewel mod (name, what it does). */
export const jewelModList = () => (listCache ??= invoke<JewelMod[]>("jewel_mods").catch(() => { listCache = null; return []; }));

type Detail = { mods?: string[]; mods_min?: number; style: CurrentStyle };
/** The saved edit, if there is one. */
export async function readJewelRule(): Promise<JewelRule | null> {
  for (const [i, label] of JM_LABELS.entries()) {
    try {
      const [, , d] = await invoke<[string, number, Detail]>("get_rule", { id: null, index: null, label });
      return { mods: d.mods ?? [], min: d.mods_min ?? 0, look: d.style, styled: i === 1 };
    } catch { /* not this one */ }
  }
  return null;
}

async function removeJewelRule() {
  for (const label of JM_LABELS) {
    try { const [id, index] = await invoke<[string, number]>("get_rule", { id: null, index: null, label }); await invoke("delete_rule", { id, index }); } catch { /* not there */ }
  }
}

/** Saves the list (no mods = no edit). A new look, else the one it had, else a purple highlight. */
export async function saveJewelRule(mods: string[], min: number, style?: CustomStyle | null, keep?: CustomStyle | null): Promise<{ label: string; warnings: string[] } | null> {
  await removeJewelRule();
  if (mods.length === 0) return null;
  const opts = { same_rarity: false, max_item_level: null, corrupted: false, mods, mods_min: Math.min(min, mods.length) };
  const look = style ?? keep;
  return look
    ? invoke("apply_custom_style", { item: JM_ITEM, style: look, opts })
    : invoke("apply_quick_action", { item: JM_ITEM, action: "highlight", colour: "purple", opts });
}
