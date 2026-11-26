/**
 * The right picture for each kind of map (he asked 23 Nov: an Originator map showed Veritania's conqueror art).
 * Names are keys in the price job's picture list (poe.ninja's own names): "Veritania Map (Tier 16)",
 * "The Enslaver Map (Tier 16)", "Blight-ravaged Map (Tier 16)", "Valdo Map"… A plain "Map (Tier N)" is the plain
 * map (the price job keeps the unmarked picture for it), which is also what Originator maps look like in game.
 */
export const CONQUERORS = ["Veritania", "Drox", "Baran", "Al-Hezmin"];
export const ELDER_GUARDIANS = ["The Enslaver", "The Eradicator", "The Constrictor", "The Purifier"];

/** Pictures for one of NeverSink's special map rules (Map tiers page, "Special maps"). */
export function tierMapIcons(typePath: string[], tier: string): string[] {
  const kind = typePath[1] ?? "";
  if (kind === "influenced") {
    if (tier === "infconquerors") return CONQUERORS.map((c) => `${c} Map (Tier 16)`);
    if (tier === "infelder") return ELDER_GUARDIANS.map((g) => `${g} Map (Tier 16)`);
    return [tier === "zanamaplow" ? "Map (Tier 10)" : "Map (Tier 16)"];
  }
  if (kind === "implicitmod") return CONQUERORS.map((c) => `${c} Map (Tier 16)`);
  if (kind === "nightmare") {
    if (tier === "valdomaps") return ["Valdo Map"];
    if (tier === "shaperguardianmaps") return ["Shaper Guardian Map"];
    return ["Nightmare Map"];
  }
  if (kind === "blighted") return [tier === "uber" ? "Blight-ravaged Map (Tier 16)" : tier === "t1" ? "Blighted Map (Tier 16)" : "Blighted Map (Tier 10)"];
  if (kind === "vaaltemple") return ["Vaal Temple Map"];
  return ["Map (Tier 16)"];
}

type MapItem = { class?: string; base_type?: string; blighted?: boolean; uber_blighted?: boolean; occupied_by?: string | null; zana_memory?: boolean };

/** The picture name for a map item (F2, price window): blighted, conqueror, Elder guardian or Originator art when it
 *  has it. "Originator Map (Tier N)" pictures come from his own maps (the game marks them; 26 Nov he asked for them):
 *  a tier the editor hasn't seen yet shows the plain map. */
export function mapItemIcon(item: MapItem): string | null {
  const base = item.base_type ?? "";
  if (!/Map\b/.test(base)) return null;
  if (item.uber_blighted) return `Blight-ravaged ${base}`;
  if (item.blighted) return `Blighted ${base}`;
  const who = item.occupied_by ?? "";
  if (who && (CONQUERORS.includes(who) || ELDER_GUARDIANS.includes(who))) return `${who} ${base}`;
  if (item.zana_memory) return `Originator ${base}`;
  return base;
}
