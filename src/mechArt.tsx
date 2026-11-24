/** The league mechanics' own symbols, as in the middle of each area on the in-game atlas tree (he asked 24 Nov: clearer than
 *  the threads). One picture of all of them, from GGG's atlas tree export (3.30): public/atlas-mastery.webp, 7 across,
 *  113 px each with about 15 px of nothing round the symbol. */
const CELL = 113, PAD = 15, ACROSS = 7, DOWN = 6;

/** Mechanic key (as in core atlas_opt MECHANICS) or area name on the atlas tree -> [column, row]. */
const AT: Record<string, [number, number]> = {
  abyss: [0, 0], incursion: [1, 0], "rogue exiles": [2, 0], "atlas anomalies": [3, 0], "viridian wildwood": [4, 0], bestiary: [5, 0], beyond: [6, 0],
  blight: [0, 1], breach: [1, 1], "the searing exarch": [2, 1], conquerors: [3, 1], delirium: [4, 1], "divination cards": [5, 1], shrines: [6, 1],
  "the shaper and elder": [0, 2], essence: [1, 2], expedition: [2, 2], gold: [3, 2], harvest: [4, 2], "sacred flora": [4, 2], betrayal: [5, 2], labyrinth: [6, 2],
  legion: [0, 3], "magic monsters": [1, 3], maps: [2, 3], memories: [3, 3], "atlas memories": [3, 3], mercenaries: [4, 3], mirage: [5, 3], "rare monsters": [6, 3],
  ritual: [0, 4], sentinels: [1, 4], settlers: [2, 4], "settlers of kalguur": [2, 4], strongboxes: [3, 4], synthesis: [4, 4], "the eater of worlds": [5, 4], threads: [6, 4],
  torment: [0, 5], ultimatum: [1, 5], bosses: [2, 5], "unique monsters": [2, 5],
};

export const hasMechArt = (m: string) => m.toLowerCase() in AT;

export function MechArt({ m, size, title }: { m: string; size: number; title?: string }) {
  const at = AT[m.toLowerCase()];
  if (!at) return null;
  const k = size / (CELL - 2 * PAD);
  return <span className="mech-art" title={title} aria-hidden={title ? undefined : true} style={{
    width: size, height: size, backgroundImage: "url(/atlas-mastery.webp)",
    backgroundSize: `${ACROSS * CELL * k}px ${DOWN * CELL * k}px`,
    backgroundPosition: `${-(at[0] * CELL + PAD) * k}px ${-(at[1] * CELL + PAD) * k}px`,
  }} />;
}
