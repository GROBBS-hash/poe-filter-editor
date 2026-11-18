export type Item = {
  item_class: string;
  rarity: string;
  name?: string | null;
  base_type: string;
  item_level?: number | null;
  stack_size?: number | null;
  quality?: number | null;
  sockets?: string | null;
  map_tier?: number | null;
  corrupted: boolean;
  mirrored: boolean;
  unidentified: boolean;
};

export type Action = "hide" | "highlight" | "louder" | "quieter";
export type Game = "Poe1" | "Poe2";

export type ProfileInfo = {
  id: string;
  name: string;
  game: Game;
  base_file: string;
  output_file: string;
  parent?: string | null;
  rules: { index: number; label: string }[];
};

export type Overview = {
  profiles: ProfileInfo[];
  active?: string | null;
  poe1_dir: string;
  poe2_dir: string;
  batch_on: boolean;
};

export const RARITY_COLOUR: Record<string, string> = {
  Normal: "#c8c8c8",
  Magic: "#8888ff",
  Rare: "#ffff77",
  Unique: "#af6025",
  Currency: "#aa9e82",
  Gem: "#1ba29b",
  "Divination Card": "#9ee2ff",
};

export const COLOURS = [
  { name: "purple", hex: "#aa3cff" },
  { name: "gold", hex: "#ffc828" },
  { name: "cyan", hex: "#00f0be" },
  { name: "red", hex: "#ff3c3c" },
  { name: "green", hex: "#46e646" },
];

export const itemTitle = (i: Item) => i.name ?? i.base_type;
export const itemSub = (i: Item) =>
  [i.name ? i.base_type : null, i.item_class, i.item_level ? `ilvl ${i.item_level}` : null,
   i.map_tier ? `T${i.map_tier}` : null, i.corrupted ? "Corrupted" : null]
    .filter(Boolean).join(" · ");
