// Dev tool: the main window in a normal browser with sample data, to check layout and styling.
// With the editor running (`npm run tauri dev`), open http://localhost:1420/preview-app.html
// Pages that need more than the sample data show empty states; Wealth and the menu are the main targets.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import App from "./App";
import AtlasOverlay from "./AtlasOverlay";
import maxrollSample from "../core/data/test_maxroll_vg1ga30g.json";
import atlasData from "../public/atlas_poe1.json";
import BuildOverlay from "./BuildOverlay";
import "./styles.css";
import { ChoiceWindow, LevelWindow, NotesWindow, SearchesWindow, SheetWindow, TrackerWindow } from "./Play";
import { choiceView, playMock } from "./preview-play";
import { emit } from "@tauri-apps/api/event";

const now = Math.floor(Date.now() / 1000);
const snap = (t: number, total: number) => ({ time: t, league: "Covens", total, divine: 217, tabs: [["Frags", total * 0.6], ["GEM", total * 0.2]], profile: "Dump tabs" });
const wealth = {
  account: "Exile#1234", has_sessid: true, league: "Covens", min: 10, minutes: 3, divine: 217, now,
  tabs: ["Frags", "BREACH", "DELI", "Div", "GEM", "Dump 1", "Dump 2"].map((name, index) => ({ index, name, kind: "PremiumStash", colour: "#c9a94a" })),
  tracked: [5, 6], profiles: [{ name: "Dump tabs", tabs: [0, 1, 4] }], active: "Dump tabs", rares: 12, rares_priced: 0, rares_todo: 12,
  snapshots: [snap(now - 7200, 160), snap(now - 3600, 141), snap(now - 60, 584)],
  missed: [["Blight Thread", 50, 133], ["Legion Thread", 18, 65.7], ["Bestiary Thread", 29, 47.8], ["Summon Carrion Golem of Hordes", 1, 21]]
    .map(([name, stack, value]) => ({ tab: "Frags", name, stack, value, x: 3, y: 2, listed: false, icon: "", places: 1 })),
  last_items: [["Frags", "Blight Thread", 30, 80], ["Frags", "Blight Thread", 20, 53], ["Frags", "Legion Thread", 18, 65.7], ["GEM", "Summon Carrion Golem of Hordes", 1, 21], ["GEM", "Vaal Grace", 1, 5], ["GEM", "Enlighten Support", 1, 160]]
    .map(([tab, name, stack, value]) => ({ tab, name, stack, value, x: 2, y: 4, listed: name === "Vaal Grace", icon: "", places: 1 })),
  sessions: [], current: { name: "Strand alch and go", league: "Covens", tabs: [5, 6], start: now - 2400, end: null, start_value: 300, last_value: 445, paused: 0, paused_at: null },
  strategies: [["Strand alch and go", 3, 2.5, 212]],
};

let savedSearches = [
  { id: 1, name: "Stygian Vise, life + res", live: true, setup: { filters: [{ enabled: true, label: "+# to maximum Life" }, { enabled: true, label: "+#% total Elemental Resistance" }] } },
  { id: 2, name: "Large Cluster Jewel: Lightning", live: false, setup: { filters: [{ enabled: true, label: "1 Added Passive Skill is Thunderstruck" }] } },
];
// Tiers → Map tiers: NeverSink-like map rules (whites T1-5, yellows T6-10, reds T11-16); hidden ones remembered here.
const mapHidden = new Set<string>(["maps_d_t1", "maps_d_t2"]);
let batchItems = [
  { item_class: "Rings", rarity: "Normal", name: null, base_type: "Iron Ring", item_level: 70, corrupted: false, mirrored: false, unidentified: false },
  { item_class: "Stackable Currency", rarity: "Currency", name: null, base_type: "Orb of Alteration", item_level: null, stack_size: 4, corrupted: false, mirrored: false, unidentified: false },
  { item_class: "Body Armours", rarity: "Unique", name: "Tabula Rasa", base_type: "Simple Robe", item_level: 68, corrupted: false, mirrored: false, unidentified: false },
];
let cjRule: { kind: string; style: Record<string, unknown> } | null = null;
let jmRule: { kind: string; mods: string[]; min: number } | null = { kind: "Highlight:", mods: ["Shimmering"], min: 0 };
let tiersCache: unknown = null;
const mapStyle = (b: string) => ({ font_size: 45, muted: false, text: b === "d" ? [255, 255, 255, 255] : [0, 0, 0, 255],
  background: b === "d" ? [20, 20, 0, 255] : b === "c" ? [235, 235, 120, 255] : [235, 235, 235, 255], border: b === "a" ? [255, 0, 0, 255] : null,
  beam: b === "d" ? "White" : "Yellow", minimap_parts: [b === "d" ? 2 : 1, b === "d" ? "White" : "Red", "Square"] });
const extrasOff = new Set<string>();
const mapTiers = () => ({
  extras: [["upgrade", "Upgrade borders", "A red border on maps above the tier you're running now, so upgrades stand out."], ["bands", "Tier group borders", "A dark border on red maps and a light one on lower maps."], ["implicit", "Implicit borders", "A gold border on maps with implicit mods (e.g. from the atlas)."]]
    .map(([key, title, what]) => ({ key, title, what, tiers: [key], on: !extrasOff.has(key), border: key === "upgrade" ? [220, 50, 0, 255] : key === "bands" ? [0, 0, 0, 255] : [255, 180, 0, 255], sample_tier: key === "implicit" ? 10 : 14 })),
  tiers: Array.from({ length: 16 }, (_, i) => { const t = i + 1; const b = t >= 14 ? "a" : t >= 11 ? "b" : t >= 6 ? "c" : "d"; const tier = `maps_${b}_t${t}`;
    return { map_tier: t, band: b, tier, name: `Maps ${b} t${t}`, visibility: mapHidden.has(tier) ? "Hide" : "Show", style: mapStyle(b), bases: [], note: "", min_stack: null }; }),
  special: [["influenced", "Influenced maps (Shaper, Elder, Conquerors, Zana)", 4], ["blighted", "Blighted maps", 3], ["nightmare", "Valdo's, nightmare & guardian maps", 3]]
    .map(([k, title, n]) => ({ type_path: ["maps", k], title, group: "Maps", shared_bases: true,
      tiers: Array.from({ length: n as number }, (_, i) => ({ tier: "t" + i, name: "Tier " + i, visibility: "Show", style: mapStyle("a"), bases: [], note: "", min_stack: null })) }))
    .concat([{ type_path: ["maps", "implicitmod"], title: "Citadel (conqueror) maps", group: "Maps", shared_bases: true, tiers: ["t1", "any"].map((tier) => ({ tier, name: tier === "t1" ? "Citadel maps, tier 14+" : "Citadel maps, lower tiers", visibility: "Show", style: { font_size: 45, muted: false, text: [145, 30, 220, 255], border: [145, 30, 220, 255], background: tier === "t1" ? [235, 220, 245, 255] : [200, 200, 200, 255], beam: "Purple", minimap_parts: [tier === "t1" ? 0 : 1, "Purple", "Square"] }, bases: [], note: "", min_stack: null })) }])
    .concat([{ type_path: ["maps", "influenced"], title: "Originator maps (Zana memories)", group: "Maps", shared_bases: true, tiers: [
      { tier: "zanamap", name: "Originator maps, tier 14+", visibility: "Show", style: { font_size: 45, muted: false, text: [100, 0, 122, 255], border: [100, 0, 122, 255], background: [255, 255, 255, 255], beam: "Red", minimap_parts: [0, "Red", "Square"] }, bases: [], note: "", min_stack: null },
      { tier: "zanamaplow", name: "Originator maps, lower tiers", visibility: "Show", style: { font_size: 45, muted: false, text: [145, 30, 220, 255], border: [145, 30, 220, 255], background: [200, 200, 200, 255], beam: "Purple", minimap_parts: [1, "Purple", "Square"] }, bases: [], note: "", min_stack: null }] }]),
});
// ?build shows the F6 build window instead, with a sample build.
const buildWin = location.search.includes("build");
const gem = (name: string, colour: string, support = false) => ({ name, level: 20, quality: 20, enabled: true, support, colour });
const pob = async () => {
  const t = await fetch("/tree_poe1.json").then((r) => r.json());
  const ids = Object.keys(t.nodes).map(Number);
  const stage = (title: string, n: number) => ({ title, tree: { version: "3.30", nodes: ids.slice(200, 200 + n), url: "" },
    groups: [{ label: "Main", slot: "Body Armour", enabled: true, gems: [gem("Arc", "b"), gem("Spell Echo", "b", true), gem("Lightning Penetration", "b", true), gem("Elemental Focus", "g", true), gem("Added Lightning Damage", "b", true), gem("Inspiration", "r", true)] },
      { label: "", slot: "Helmet", enabled: true, gems: [gem("Herald of Ash", "r"), gem("Arcane Surge", "b", true)] },
      { label: "", slot: "Helmet", enabled: true, gems: [gem("Flame Dash", "b"), gem("Faster Casting", "b", true)] },
      { label: "Swap", slot: "Weapon 1 Swap", enabled: true, gems: [gem("Clarity", "b")] }],
    gear: [{ slot: "Body Armour", rarity: "rare", name: "Doom Shelter", base: "Vaal Regalia", lines: ["Sockets: B-B-B-G-B-R", "+90 to maximum Life"], copy: "x" },
      { slot: "Helmet", rarity: "rare", name: "Doom Crown", base: "Hubris Circlet", lines: ["Sockets: R-B B-B", "+80 to maximum Life"], copy: "x" }, { slot: "Flask 1", rarity: "unique", name: "Coruscating Elixir", base: "Ruby Flask", lines: [], copy: "x" }, { slot: "Flask 2", rarity: "magic", name: "Examiner's Topaz Flask of the Rainbow", base: "Topaz Flask", lines: [] }],
    jewels: [{ node: 7960, notables: ["Doryani's Lesson", "Inspired Oppression", "Overshock"], item: { slot: "Jewel socket 7960", rarity: "rare", name: "8 Passive Only! Not 9!", base: "Large Cluster Jewel", lines: ["Adds 8 Passive Skills", "1 Added Passive Skill is Overshock"], copy: "x" } },
      { node: 26196, notables: [], item: { slot: "Jewel socket 26196", rarity: "unique", name: "Brutal Restraint", base: "Timeless Jewel", lines: ["Denoted service of 7000 dekhara in the akhara of Balbala"], copy: "x" } },
      { node: null, notables: [], item: { slot: "Belt Abyssal Socket 1", rarity: "rare", name: "Bleed + CB Avoidance Jewel", base: "Hypnotic Eye Jewel", lines: ["+12% chance to Avoid Bleeding"] } }] });
  return { build: { class: "Witch", ascendancy: "Elementalist", level: 50, active: 1, stages: [stage("Start of Maps", 90), stage("End of Early Maps", 120)] }, stage: 1, link: "", follow: false };
};
// Atlas strategies: maxroll's "Leaguestart Shrine/Magic Atlas" (variant 2 is a full tree) with sample notes.
// Add ?atlas-window to the address to see the F7 window instead of the main window.
const mxVariant = JSON.parse((maxrollSample as { data: string }).data).profiles[1].atlas.variants[0];
// Steps the way core/src/atlas.rs makes them (a step ends at a notable/keystone or where the next point isn't joined).
const atlasSteps = (pts: number[]) => {
  const nodes = atlasData.nodes as unknown as Record<string, [number, number, number, string]>;
  const adj = new Map<number, Set<number>>();
  for (const e of atlasData.edges as number[][]) { for (const [a, b] of [[e[0], e[1]], [e[1], e[0]]]) { if (!adj.has(a)) adj.set(a, new Set()); adj.get(a)!.add(b); } }
  const out: { name: string; node: number; nodes: number[]; total: number }[] = [];
  let cur: number[] = [], total = 0;
  pts.forEach((n, i) => {
    cur.push(n);
    const next = pts[i + 1];
    if (next === undefined || !adj.get(n)?.has(next) || [1, 2].includes(nodes[n]?.[2])) { total += cur.length; out.push({ name: nodes[n]?.[3] ?? "", node: n, nodes: cur, total }); cur = []; }
  });
  return out;
};
const note = (title: string, lines: [string, string][]) => ({ title, lines: lines.map(([kind, text]) => ({ kind, depth: 0, text })) });
let atlas = { active: 1, variant: 0, list: [
  { id: 1, name: "Leaguestart Shrine/Magic Atlas", site: "maxroll", link: "https://maxroll.gg/poe/farming-strategy-planner/vg1ga30g", author: "palsteron", imported: now,
    device: { maps: ["City Square Map"], threads: ["Ritual ×8", "Breach", "Incursion"], rolls: ["30% increased Pack size"], text: "Activate the Sacred Flora before doing ritual." },
    notes: [note("About", [["p", "Shrines and magic packs early in the league, cheap to run."]]), note("How to play", [["h", "Maps"], ["li", "Run any tier 16 with 30% pack size."], ["li", "Click every shrine."]])],
    variants: [{ name: "Shrines", nodes: [29045, ...mxVariant.history], steps: atlasSteps(mxVariant.history), choices: Object.entries(mxVariant.masteries).map(([n, e]) => [Number(n), e]),
      notes: [note("Map device & setup", [["li", "Breach Thread · Ritual Thread · Abyss Thread"]])] }],
    own_notes: "Sell shrine maps in bulk." },
  { id: 2, name: "My essence tree", site: "official", link: "", author: "", imported: now, device: null, notes: [], own_notes: "",
    variants: [{ name: "Atlas tree", nodes: [29045, ...mxVariant.history.slice(0, 40)], steps: atlasSteps(mxVariant.history.slice(0, 40)), choices: [], notes: [] }] },
] };
const atlasWindow = location.search.includes("atlas-window");
// ?play=level (tracker, searches, sheet, notes, choice) shows that play window with sample data (preview-play.ts).
const playWin = new URLSearchParams(location.search).get("play") ?? (location.search.includes("sheet") ? "sheet" : null);
const sheetWin = playWin != null;
mockWindows(playWin ? playWin : atlasWindow ? "atlas" : buildWin ? "build" : "main");
// My edits + saved looks: a few edits, two using "My chase look", one switched off.
const lookStyle = (rgb: number[], beam: string | null) => ({ font_size: 45, text: [...rgb, 255], border: [...rgb, 255], background: [20, 20, 20, 240],
  beam: beam ? { colour: beam, temp: false } : null, minimap: beam ? { size: 1, colour: beam, shape: "Star" } : null, sound: { kind: "Default" }, label_hidden: false });
let looks = [{ name: "My chase look", style: lookStyle([255, 60, 60], "Red") }, { name: "Good stuff", style: lookStyle([255, 200, 40], "Yellow") }];
const curStyle = (s: { font_size: number; text: number[]; border: number[] | null; background: number[]; beam: { colour: string } | null }) =>
  ({ font_size: s.font_size, text: s.text, border: s.border, background: s.background, beam: s.beam?.colour ?? null, muted: false });
const edit = (base: string, cls: string, look: string | null, off = false, show = true) => {
  const st = looks.find((l) => l.name === look)?.style ?? lookStyle([180, 140, 255], null);
  return { label: `${show ? "Style" : "Hide"}: ${base}`, off, look, detail: { label: `${show ? "Style" : "Hide"}: ${base}`, show, item_class: cls, base_type: base, rarity: null, style: curStyle(st), look, off } };
};
let rules = [edit("Divine Orb", "Stackable Currency", "My chase look"), edit("Mirror of Kalandra", "Stackable Currency", "My chase look"),
  edit("Chaos Orb", "Stackable Currency", null, true), edit("Iron Ring", "Rings", null, false, false)];
const relook = () => { rules = rules.map((r) => r.look ? edit(r.detail.base_type, r.detail.item_class, looks.some((l) => l.name === r.look) ? r.look : null, r.off, r.detail.show) : r); };
mockIPC((cmd, args) => {
  if (cmd === "overview") return { profiles: [{ id: "mid", name: "Mid-Game", game: "Poe1", base_file: "", output_file: "Mid-Game.filter", rules: rules.map((r, index) => ({ index, shadowed_by: null, ...r })) }], active: "mid", poe1_dir: "", poe2_dir: "", batch_on: false };
  if (cmd === "list_looks") return looks.map((l) => ({ ...l, used_by: rules.filter((r) => r.look === l.name).length }));
  if (cmd === "save_look") {
    const a = args as { look: { name: string; style: ReturnType<typeof lookStyle> }; oldName: string | null };
    if (looks.some((l) => l.name.toLowerCase() === a.look.name.toLowerCase() && l.name !== a.oldName)) throw `You already have a look called “${a.look.name}”`;
    looks = a.oldName ? looks.map((l) => l.name === a.oldName ? a.look : l) : [...looks, a.look];
    rules = rules.map((r) => r.look === a.oldName ? { ...r, look: a.look.name } : r); relook(); return [];
  }
  if (cmd === "delete_look") { const n = (args as { name: string }).name; looks = looks.filter((l) => l.name !== n); rules = rules.map((r) => r.look === n ? { ...r, look: null, detail: { ...r.detail, look: null } } : r); return null; }
  // Share packs (Filters page).
  if (cmd === "export_pack") return { code: "LODE1:eNrtWm1v2zYQ_iuEPrexJcf50m9pm24BsqFo0hRY0Q-0RNtcJFIjqThO0P-" + "x".repeat(300), file_name: "Mid-Game.lodestone",
    summary: { name: "Mid-Game", game: "Poe1", strictness: 2, edits: rules.length, edits_off: rules.filter((r) => r.off).length, looks: looks.map((l) => l.name), theme: "Ember", prices: true } };
  if (cmd === "read_pack") { const t = String((args as { text?: string }).text ?? ""); if (!t.startsWith("LODE1:")) throw "That isn't a share code or pack file (codes start with LODE1:)";
    return { name: "Sam's Strict", game: "Poe1", strictness: 3, edits: 31, edits_off: 2, looks: ["Chase", "Quiet"], theme: "Ember", prices: true }; }
  if (cmd === "import_pack") return "sams-strict";
  if (cmd === "set_rule_off") { const a = args as { index: number; off: boolean }; rules = rules.map((r, i) => i === a.index ? { ...r, off: a.off, detail: { ...r.detail, off: a.off } } : r); return []; }
  // An expired pathofexile.com login: the stash scan says so (shows the "Log in again" message). The login window can't open here.
  if (cmd === "wealth_scan") throw "Your pathofexile.com login has expired: press \"Log in again\" (or \"Log in with pathofexile.com\" in Settings → Map tracker).";
  if (cmd === "poe_login") return null;
  // Sell in bulk: a few groups, the basket priced like the app (fraction "total/count" in chaos, divines from 1 div each).
  if (cmd === "bulk_get") return { divine: 180, league: "Keepers", scanned: Math.floor(Date.now() / 1000) - 1500, profile: "", has_sessid: true,
    groups: ["Currency", "Essences", "Div cards", "Fragments", "Lifeforce", "Threads"],
    items: ([["Divine Orb", "Currency", 3, 180, "$$$"], ["Orb of Alchemy", "Currency", 380, 0.3, "Dump 1, Currency"], ["Orb of Fusing", "Currency", 140, 0.45, "Currency"],
      ["Deafening Essence of Contempt", "Essences", 18, 6, "Dump 1"], ["Deafening Essence of Greed", "Essences", 24, 4, "Dump 1, Dump 2"], ["Shrieking Essence of Hatred", "Essences", 40, 1.2, "Dump 2"], ["Screaming Essence of Woe", "Essences", 3, 0.5, "Dump 2"],
      ["The Wrath", "Div cards", 9, 11, "Cards"], ["Rain of Chaos", "Div cards", 30, 0.4, "Cards"], ["Fragment of the Hydra", "Fragments", 4, 30, "Frags"],
      ["Wild Crystallised Lifeforce", "Lifeforce", 4200, 0.02, "Dump 2"], ["Abyss Thread", "Threads", 12, 3, "Dump 1"]] as [string, string, number, number, string][])
      .map(([name, group, have, each, tabs]) => ({ name, group, have, each, icon: "", tabs, set: name === "The Wrath" ? 8 : name === "Rain of Chaos" ? 8 : null })) };
  if (cmd === "bulk_basket") {
    const a = args as { picks: { name: string; count: number; each: number }[]; discount: number; title: string; character: string };
    const gcd = (x: number, y: number): number => (y ? gcd(y, x % y) : Math.max(1, x));
    const rows = a.picks.map((p) => {
      const each = p.each * (1 - a.discount / 100);
      if (each >= 180) { const d = Math.round(each / 18) / 10; return { name: p.name, count: p.count, note: { amount: String(d), currency: "divine", each: d * 180 }, total: d * 180 * p.count }; }
      const t = Math.max(1, Math.round(each * p.count)); const g = gcd(t, p.count);
      return { name: p.name, count: p.count, note: { amount: p.count / g === 1 ? String(t / g) : `${t / g}/${p.count / g}`, currency: "chaos", each: t / p.count }, total: t };
    });
    const total = rows.reduce((s, r) => s + r.total, 0);
    const txt = (c: number) => (c >= 180 ? `${Math.round(c)}c (${+(c / 180).toFixed(1)} div)` : `${Math.round(c)}c`);
    const post = rows.length ? [`WTS Keepers · ${a.title || "Bulk"}`, ...rows.map((r) => `${r.count}x ${r.name} · ${Math.round(r.total)}c`),
      `All for ${txt(total)}${a.discount ? ` · ${a.discount}% under market` : ""}`, ...(a.character ? [`IGN: @${a.character}`] : [])].join("\n") : "";
    return { rows, total, total_text: txt(total), post };
  }
  if (cmd === "bulk_live") return new Promise((r) => setTimeout(() => r({ chaos: (args as { each: number }).each * ((args as { name: string }).name.includes("Lifeforce") ? 50 : 0.92), offers: 20, url: "", fair: !(args as { name: string }).name.includes("Lifeforce") }), 400));
  if (cmd === "wealth_get") return wealth;
  if (cmd === "buy_base_item") return { total: 3480, ilvl: 75, notables: [["Doryani's Lesson", 68], ["Inspired Oppression", 75], ["Overshock", 50]] };
  if (cmd === "get_price_settings") return { settings: { enabled: false, groups: [{ group: "DivinationCards", enabled: true, tiers: [], hide_below: null }, { group: "Uniques", enabled: true, tiers: [], hide_below: null }] }, info: { unit: "chaos", league: "Keepers", updated: new Date().toISOString(), count: 18432 } };
  if (cmd === "price_count_below") return [Math.round(Math.min(480, (args as { limit: number }).limit * 60)), 512];
  if (cmd === "set_price_settings") return [];
  if (cmd === "wealth_find") {
    const all = [
      { tab_index: 28, tab: "TENDY", name: "Woe Buckle Stygian Vise", kind: "Rare", stack: 1, x: 3, y: 1, icon: "", ilvl: 84, note: "", value: 210,
        mods: "Item Level: 84\n+32% to Chaos Resistance\n+47 to maximum Energy Shield\n38% increased Elemental Damage with Attack Skills" },
      { tab_index: 5, tab: "$$$", name: "Divine Orb", kind: "Currency", stack: 9, x: 0, y: 0, icon: "", ilvl: 0, note: "", value: 1953, mods: "Stack Size: 9/20" },
      { tab_index: 18, tab: "GEM", name: "Vaal Grace", kind: "Gem", stack: 1, x: 5, y: 0, icon: "", ilvl: 0, note: "in Rage Touch Paladin Gloves", value: null, mods: "Level: 20" },
    ];
    const q = String((args as { query: string }).query).toLowerCase().trim();
    const hits = q ? all.filter((f) => (f.name + f.kind + f.mods).toLowerCase().includes(q)) : [];
    return { items: 4231, time: now, league: "Covens", total: hits.length, hits };
  }
  if (cmd === "map_tool_get") return { source: "poe.re (preview)", bad: [1], wants: { quantity: null, pack_size: null, rarity: null, more_maps: null, more_currency: null, more_scarabs: null, more_cards: null, tier_min: null, kind: "any", corrupted_ok: true },
    mods: [{ id: 1, text: "Monsters reflect 18% of Elemental Damage", token: "s def", scary: 900, nightmare: false, trade_id: null, danger: "Elemental reflect" },
      { id: 2, text: "Players cannot Regenerate Life, Mana or Energy Shield", token: "t reg", scary: 800, nightmare: false, trade_id: null, danger: "No regen" }] };
  if (cmd === "map_tool_regex") return "\"!s def\"";
  // Real icons when the editor is running (vite serves the app's icon list), else none.
  // public/_icons.json (git-ignored) when present: a picture list with the extra sources merged in, else his own.
  if (cmd === "get_icons") return fetch("/_icons.json").then((r) => r.ok ? r.json() : Promise.reject()).catch(() => fetch("/preview-data/icons").then((r) => r.json())).catch(() => ({}));
  if (cmd === "get_settings") return { key_atlas: "F7" };
  // Price check page: saved searches (Live on/off and delete work in the preview).
  if (cmd === "saved_list") return savedSearches;
  if (cmd === "saved_live") { const a = args as { id: number; on: boolean }; savedSearches = savedSearches.map((s) => s.id === a.id ? { ...s, live: a.on } : s); return savedSearches; }
  if (cmd === "saved_delete") { const a = args as { id: number }; savedSearches = savedSearches.filter((s) => s.id !== a.id); return savedSearches; }
  { const p = playMock(cmd); if (p !== undefined) return p; }
  if (cmd === "price_names") return Object.fromEntries((args as { names: string[] }).names.map((n, i) => [n, n === "Divine Orb" ? 260 : [1, 1, 1, 1, 2, 2, 3, 4, 6, 9, 14, 30, 90, 600][i % 14]]));
  if (cmd === "atlas_list") return atlas;
  if (cmd === "atlas_use") { const a = args as { id: number; variant: number }; atlas = { ...atlas, active: a.id, variant: a.variant }; return atlas; }
  if (cmd === "atlas_rename") { const a = args as { id: number; name: string }; atlas = { ...atlas, list: atlas.list.map((s) => s.id === a.id ? { ...s, name: a.name } : s) }; return atlas; }
  if (cmd === "atlas_own_notes") { const a = args as { id: number; text: string }; atlas = { ...atlas, list: atlas.list.map((s) => s.id === a.id ? { ...s, own_notes: a.text } : s) }; return atlas; }
  if (cmd === "atlas_delete") { const a = args as { id: number }; atlas = { ...atlas, list: atlas.list.filter((s) => s.id !== a.id) }; return atlas; }
  if (cmd === "atlas_import") throw "The preview can't reach maxroll: try it in the app";
  if (cmd === "atlas_save_tree") {
    const a = args as { id: number; variant: number; nodes: number[]; choices: [number, number][]; asNew: string };
    const pts = a.nodes.filter((n) => n !== 29045);
    const edit = (s: (typeof atlas.list)[number]) => ({ ...s, variants: s.variants.map((x, i) => i === a.variant ? { ...x, nodes: [29045, ...pts], steps: atlasSteps(pts), choices: a.choices } : x) }) as typeof s;
    const src = atlas.list.find((s) => s.id === a.id)!;
    atlas = a.asNew ? { ...atlas, active: 77, list: [...atlas.list, { ...edit(src), id: 77, name: a.asNew }] }
      : { ...atlas, active: a.id, list: atlas.list.map((s) => s.id === a.id ? edit(s) : s) };
    return atlas;
  }
  if (cmd === "atlas_optimise" || cmd === "atlas_save_plan") {
    // Stand-in for core atlas_opt.rs: maxroll's sample tree, shorter for fewer points, minus blocked nodes.
    const ask = (args as { ask: { mechanics: string[]; priority: string; lock: number[]; block: number[] } }).ask;
    const n = ask.priority === "fewest" ? 60 : ask.priority === "balanced" ? 90 : mxVariant.history.length;
    const pts = [...new Set([...ask.lock, ...mxVariant.history.slice(0, n)])].filter((x: number) => !ask.block.includes(x));
    const nodes = atlasData.nodes as unknown as Record<string, [number, number, number, string]>;
    const notable = pts.filter((x: number) => nodes[x]?.[2] === 1);
    const work = (x: number, i: number) => ({ node: x, name: nodes[x][3], value: 4 - i * 0.1, why: `${ask.mechanics[0]} node · taken by ${5 - (i % 4)} of 7 trees` });
    const plan = { nodes: [29045, ...pts], steps: atlasSteps(pts), choices: [], points: pts.length, budget: 150,
      top: notable.slice(0, 10).map(work), weak: notable.slice(10, 12).map((x: number, i: number) => ({ ...work(x, i), why: "on the way to better nodes" })), trees_used: 7,
      notes: ["Eater of Worlds side only: a map has one Eldritch influence"] };
    if (cmd === "atlas_optimise") return plan;
    atlas = { ...atlas, active: 99, list: [...atlas.list, { ...atlas.list[0], id: 99, name: `${ask.mechanics.join(" + ")} (optimiser)`, site: "optimiser", link: "" }] };
    return atlas;
  }
  if (cmd === "pob_get") return pob();
  if (cmd === "build_tree_opt") return [true, "P"];
  if (cmd === "list_sounds") return [];
  // Crafting page: public/_craft_pool.json / _craft_ways.json when present (cargo run --example craftof with CRAFT_JSON=public).
  if (cmd === "craft_bases") return ["Eternal Burgonet", "Hubris Circlet", "Vaal Regalia", "Unset Ring", "Paladin Boots"];
  if (cmd === "craft_pool") return fetch("/_craft_pool.json").then((r) => r.ok ? r.json() : null).catch(() => null);
  if (cmd === "craft_ways") return fetch("/_craft_ways.json").then((r) => r.ok ? r.json() : []).catch(() => []);
  if (cmd === "craft_start") return fetch("/_craft_start.json").then((r) => r.ok ? r.json() : null).catch(() => null);
  if (cmd === "craft_plan") return fetch("/_craft_ways.json").then((r) => r.ok ? r.json() : []).catch(() => []);
  // public/_craft_routes.json: cargo run --example routesof with CRAFT_JSON=public.
  if (cmd === "craft_routes") return fetch("/_craft_routes.json").then((r) => r.ok ? r.json() : null).catch(() => null);
  if (cmd === "craft_buy") return new Promise((ok) => setTimeout(() => ok({ summary: [380, 520], total: 37, url: "https://www.pathofexile.com/trade", divine: 221, listings: [] }), 600));
  // Tiers sections: public/_tiers.json when present (tierdump --json from a real filter), else none.
  if (cmd === "get_tiers") return tiersCache ? JSON.parse(JSON.stringify(tiersCache))
    : fetch("/_tiers.json").then((r) => r.ok ? r.json() : []).catch(() => []).then((s) => (tiersCache = s, JSON.parse(JSON.stringify(s))));
  // Step condition chips: change the cached copy (words are rough here; the real app words them in Rust).
  if (cmd === "set_tier_conds") {
    const a = args as { typePath: string[]; tier: string; set: Record<string, string>; undo: string[] };
    type C = { key: string; value: string; words: string; kind: string };
    const sec = (tiersCache as { type_path: string[]; tiers: { tier: string; conds: C[]; changed?: string[] }[] }[] | null)
      ?.find((s) => s.type_path.join(">") === a.typePath.join(">"));
    const t = sec?.tiers.find((x) => x.tier === a.tier);
    if (t) {
      t.changed = [...new Set([...(t.changed ?? []).filter((k) => !a.undo.includes(k)), ...Object.keys(a.set)])];
      for (const [k, v] of Object.entries(a.set)) {
        t.conds = t.conds.filter((c) => c.key !== k);
        if (v) t.conds.push({ key: k, value: v, words: `${k} ${v}`.replace(">= ", "").replace("True", "yes"), kind: k === "Rarity" ? "rarity" : ["True", "False"].includes(v) ? "yesno" : "number" });
      }
    }
    return [];
  }
  // Batch mode: three items collected in game.
  if (cmd === "batch_list") return batchItems;
  if (cmd === "batch_forget") { const k = (args as { keys: string[] | null }).keys; batchItems = k ? batchItems.filter((i) => !k.includes(`${i.rarity}|${i.base_type}|${i.rarity === "Unique" ? i.name ?? "" : ""}`)) : []; return null; }
  if (cmd === "item_status") {
    const it = (args as { item: { base_type: string } }).item;
    // Like his Red-Maps: an F2 edit on every rare Tier 16 map wins over the Map tiers looks ("Your edit wins").
    if (it.base_type === "Map (Tier 16)") return { area_level: 83, stackable: false, status: { assumptions: [], by_stack: [], verdict: { visibility: "Show", reason: "",
      by_user_edit: true, uncertain: [], decided_by: "Style: Rare Map (Tier 16)",
      style: { font_size: 45, text: [145, 30, 220, 255], border: [145, 30, 220, 255], background: [255, 255, 255, 255] },
      source: { kind: "edit", label: "Style: Rare Map (Tier 16)", type_path: [], tier: null, section: null, tier_name: null } } } };
    const hidden = it.base_type === "Iron Ring";
    // Like his Omen of Return (24 Nov): hidden by an edit for just this one item.
    if (hidden) return { area_level: 83, stackable: false, status: { assumptions: [], by_stack: [], verdict: { visibility: "Hide", reason: "", by_user_edit: true, uncertain: [],
      decided_by: "Hide: Iron Ring", style: { font_size: 40, text: [255, 255, 255, 255], border: null, background: [0, 0, 0, 200] },
      source: { kind: "edit", label: "Hide: Iron Ring", type_path: [], tier: null, section: null, tier_name: null, only_base: "Iron Ring" } } } };
    return { area_level: 83, stackable: false, status: { assumptions: [], by_stack: [], verdict: { visibility: hidden ? "Hide" : "Show", reason: "", by_user_edit: false, uncertain: [], decided_by: "",
      style: { font_size: 40, text: [255, 255, 255, 255], border: null, background: [0, 0, 0, 200] },
      source: { kind: "neversink", label: "", type_path: ["currency"], tier: "t4chaos", section: "Currency", tier_name: "Tier 4 · about a Chaos Orb", conds: ["stackable currency"] } } } };
  }
  // "Jewels with my mods" on the Jewels page (24 Nov): one saved edit holding the mod names.
  // Tiers → Threads (24 Nov): poe.ninja's Thread prices, cheapest first.
  if (cmd === "get_threads") return { unit: "chaos", limit: null, stack: null,
    shows: { "Incursion Thread": { from: 2, kind: "edit", label: "Hide: Incursion Thread (stacks under 2)", rule: 0, parent: null }, "Breach Thread": { from: 1, kind: "", label: "", rule: null, parent: null } }, items: [["Expedition Thread", 0.28], ["Betrayal Thread", 0.3], ["Incursion Thread", 0.31],
    ["Blight Thread", 0.39], ["Ritual Thread", 0.41], ["Breach Thread", 0.46], ["Ultimatum Thread", 1.62], ["Bestiary Thread", 2.62], ["Legion Thread", 5.28], ["Abyss Thread", 5.53]] };
  if (cmd === "jewel_mods") return [
    { name: "Shimmering", text: "(6-8)% increased maximum Energy Shield", prefix: true, tags: [] },
    { name: "Vivid", text: "(5-7)% increased maximum Life", prefix: true, tags: [] },
    { name: "of Haste", text: "(3-5)% increased Attack Speed", prefix: false, tags: [] },
    { name: "of Berserking", text: "(2-4)% increased Attack and Cast Speed", prefix: false, tags: [] }];
  if (cmd === "get_rule" && String((args as { label?: string }).label ?? "").includes("Jewels with my mods")) {
    const label = String((args as { label: string }).label);
    if (!jmRule || !label.startsWith(jmRule.kind)) throw "not there";
    return ["preview", 1, { label, show: true, item_class: "Jewels", base_type: "", mods: jmRule.mods, mods_min: jmRule.min,
      style: { text: [255, 255, 255, 255], border: [175, 96, 255, 255], background: [60, 20, 90, 255], font_size: 40, muted: false } }];
  }
  if (cmd === "delete_rule" && (args as { index: number }).index === 1) { jmRule = null; return null; }
  if ((cmd === "apply_quick_action" || cmd === "apply_custom_style") && ((args as { opts?: { mods?: string[] } }).opts?.mods?.length ?? 0) > 0) {
    const o = (args as { opts: { mods: string[]; mods_min: number } }).opts;
    jmRule = { kind: cmd === "apply_custom_style" ? "Style:" : "Highlight:", mods: o.mods, min: o.mods_min };
    return { label: "ok", warnings: [] };
  }
  // The "Corrupted rare jewels" switch on the Jewels page: one saved edit, highlighted or restyled.
  if (cmd === "get_rule" && String((args as { label?: string }).label ?? "").includes("Corrupted Rare every Jewels")) {
    const label = String((args as { label: string }).label);
    if (!cjRule || !label.startsWith(cjRule.kind)) throw "That edit lives in another profile";
    return ["preview", 0, { label, show: true, item_class: "Jewels", base_type: "", style: cjRule.style }];
  }
  if (cmd === "delete_rule") { cjRule = null; return null; }
  if (cmd === "apply_quick_action" || cmd === "apply_custom_style") {
    const a = args as { action?: string; style?: { text: number[]; border: number[] | null; background: number[]; font_size: number } };
    if (a.action === "hide") cjRule = null;
    else cjRule = a.style
      ? { kind: "Style:", style: { text: a.style.text, border: a.style.border, background: a.style.background, font_size: a.style.font_size, muted: false } }
      : { kind: "Highlight:", style: { text: [255, 255, 255, 255], border: [175, 96, 255, 255], background: [60, 20, 90, 255], font_size: 40, muted: false } };
    return { label: "ok", warnings: [] };
  }
  if (cmd === "get_map_tiers") return mapTiers();
  // Themes page: three built-in themes, six value levels each (Chase → Junk).
  if (cmd === "list_themes") {
    const lv = (h: number[]) => [0.42, 0.3, 0.2, 0.11, 0.05, 0].map((t, i) => ({
      text: h.map((c) => Math.min(255, Math.round(c * (1 - i * 0.12)))).concat(255), border: i < 3 ? h.concat(255) : null,
      background: h.map((c) => Math.floor(c * t)).concat([255, 245, 230, 210, 180, 140][i]) }));
    return { builtin: [{ name: "Classic PoE", levels: lv([255, 150, 60]) }, { name: "Ember", levels: lv([255, 150, 60]) }, { name: "Dracula", levels: lv([120, 200, 255]) }, { name: "NeverSink: Abyss", levels: lv([120, 230, 130]), neversink_style: "poe1_abyss", game: "poe1" }], custom: [] };
  }
  // Its cards: real drops per theme (dev: dump his with THEME_CARDS_OUT=.claude/theme-cards.json, see preview.rs).
  if (cmd === "theme_cards") return fetch("/.claude/theme-cards.json").then((r) => r.json());
  // The farm finder (dev: FARM_OUT=.claude/farm.json, see core farm.rs).
  if (cmd === "farm_find") { const want = (args as { ask: { want: string[] } }).ask.want ?? [];
    return fetch(want.length ? `/.claude/farm-${want.join("+")}.json` : "/.claude/farm.json").then((r) => r.json()); }
  // Levelling gear page: a level 50 Elementalist and a few levelling uniques.
  if (cmd === "gear_setup") return { character: ["Witch", "Elementalist", 50], league: "Keepers", gems: { Body: ["Arc", "Spell Echo"] },
    stats: [{ id: "life", name: "Life" }, { id: "resistances", name: "Resistances" }, { id: "movement_speed", name: "Movement speed" }, { id: "spell_damage", name: "Spell damage" }] };
  if (cmd === "gear_picks") {
    const u = (name: string, base: string, cls: string, level: number, slot: string, mods: string[], matched: string[], price: number | null) =>
      ({ unique: { name, base, class: cls, level, mods, stats: matched }, slot, matched, wearable: level <= 50, trade: "https://www.pathofexile.com/trade", price });
    return [
      u("Goldrim", "Leather Cap", "Helmets", 1, "Helmet", ["+30 to Evasion Rating", "+40% to all Elemental Resistances"], ["resistances"], 1),
      u("Seven-League Step", "Rawhide Boots", "Boots", 1, "Boots", ["50% increased Movement Speed"], ["movement_speed"], 85),
      u("Tabula Rasa", "Simple Robe", "Body Armours", 1, "Body", ["Item has 6 White Sockets", "6 linked"], [], 12),
      u("Lochtonial Caress", "Iron Gauntlets", "Gloves", 10, "Gloves", ["10% increased Attack and Cast Speed", "+15 to maximum Life"], ["life"], 1),
      u("Wanderlust", "Wool Shoes", "Boots", 1, "Boots", ["20% increased Movement Speed", "Cannot be Frozen"], ["movement_speed"], 1),
    ];
  }
  if (cmd === "get_history") return [{ index: 0, at: Math.floor(Date.now() / 1000) - 600, what: "Hid normal Two Hand Maces" }];
  if (cmd === "search_items") { const q = String((args as { q?: string })?.q ?? "").toLowerCase(); return [["Divine Orb", "Stackable Currency"], ["Divine Vessel", "Map Fragments"], ["Divine Life Flask", "Life Flasks"], ["Chaos Orb", "Stackable Currency"], ["Mageblood", "Belts"]].filter(([b]) => b.toLowerCase().includes(q)).map(([base_type, item_class]) => ({ base_type, item_class })); }
  // Presets page (Find view="presets"): a few league-mechanic presets.
  if (cmd === "list_farm_presets") return [
    { id: "breach", title: "Breach", hint: "Splinters, blessings and breach rings stand out", count: 14 },
    { id: "expedition", title: "Expedition", hint: "Artifacts and logbooks louder", count: 9 },
    { id: "delirium", title: "Delirium", hint: "Orbs and simulacrum splinters highlighted", count: 21 },
    { id: "harvest", title: "Harvest", hint: "Lifeforce stacks louder", count: 4 },
  ];
  if (cmd === "set_tiers") { const a = args as { tiers: string[]; show: boolean | null; clearStyle?: boolean; reset?: boolean }; for (const t of a.tiers) { if (a.clearStyle) extrasOff.add(t); else if (a.reset) extrasOff.delete(t); } for (const t of a.tiers) { if (a.show === false) mapHidden.add(t); else if (a.show) mapHidden.delete(t); } return []; }
  return null;
}, { shouldMockEvents: true });

try { if (!localStorage.getItem("tab")) localStorage.setItem("tab", "wealth"); } catch { /* fine */ }
if (!buildWin && !atlasWindow && !sheetWin) document.body.classList.add("main");
if (sheetWin) document.body.classList.add("overlay");
// For checking pages from the browser console: await __go("Themes") opens that sidebar page.
(window as unknown as { __go: (n: string) => Promise<boolean> }).__go = async (name: string) => {
  const el = Array.from(document.querySelectorAll<HTMLButtonElement>(".side button")).find((b) => b.textContent?.trim().startsWith(name));
  el?.click(); await new Promise((r) => setTimeout(r, 900)); window.scrollTo(0, 0); return !!el;
};
if (buildWin) document.body.classList.add("overlay");
if (atlasWindow) document.body.classList.add("overlay");
const PLAY_WINS: Record<string, () => React.ReactElement> = { level: LevelWindow, tracker: TrackerWindow, searches: SearchesWindow, sheet: SheetWindow, notes: NotesWindow, choice: ChoiceWindow };
function PlayPreview() { const W = PLAY_WINS[playWin ?? ""] ?? SheetWindow; if (playWin === "choice") setTimeout(() => emit("choice-read", choiceView), 300); const [w, h] = ({ level: [380, 440], tracker: [280, 360], searches: [320, 380], sheet: [900, 600], notes: [360, 420], choice: [380, 480] } as Record<string, number[]>)[playWin ?? ""] ?? [900, 600]; return <div style={{ width: w, height: h, position: "relative", transform: "translateZ(0)" }}><style>{`.bo{height:${h}px !important}`}</style><W /></div>; }
createRoot(document.getElementById("root")!).render(playWin ? <PlayPreview /> : atlasWindow ? <AtlasOverlay /> : buildWin ? <BuildOverlay /> : <App />);
