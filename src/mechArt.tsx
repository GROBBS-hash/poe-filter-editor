/** League mechanics' pictures, picked by him from a sheet (24 Nov): the game's own minimap icons (what you see in a map),
 *  except Abyss, whose minimap icon is only a green glow: Exile UI's. Files in public/mech (art © Grinding Gear Games;
 *  Abyss from Exile UI, MIT). Mechanics with no minimap icon (bosses, rare monsters…) keep their item pictures. */
const FILE: Record<string, string> = {
  abyss: "abyss.png", bestiary: "bestiary.webp", betrayal: "betrayal.webp", blight: "blight.webp", breach: "breach.webp", delirium: "delirium.webp",
  essence: "essence.webp", expedition: "expedition.webp", harvest: "harvest.webp", incursion: "incursion.webp", legion: "legion.webp",
  ritual: "ritual.webp", ultimatum: "ultimatum.webp", shrines: "shrines.webp", strongboxes: "strongboxes.webp", mirage: "mirage.webp",
  mercenaries: "mercenaries.webp", settlers: "settlers.webp",
};

export const hasMechArt = (m: string) => m.toLowerCase() in FILE;

export function MechArt({ m, size, title }: { m: string; size: number; title?: string }) {
  const f = FILE[m.toLowerCase()];
  return f ? <img className="mech-art" src={`/mech/${f}`} alt="" title={title} width={size} height={size} /> : null;
}
