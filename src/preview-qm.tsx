// Dev tool: the F2 pop-up (ItemPanel) in a browser with sample data. With the editor running, open
// http://localhost:1420/preview-qm.html  (?state=hide | icon | show picks how the sample item is styled).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import ItemPanel from "./ItemPanel";
import "./styles.css";

const params = new URLSearchParams(location.search);
const state = params.get("state") ?? "hide";
// ?editor=1: the panel as the main window shows it (Find / Tiers), not over the game.
const editor = params.get("editor") === "1";
const style = { font_size: 40, text: [255, 255, 255, 255], border: null, background: [0, 0, 0, 200], beam: null, muted: false, label_hidden: state === "icon" };
const reply = {
  stackable: true, area_level: 83, game_filter: null,
  status: { assumptions: [], by_stack: [],
    verdict: { visibility: state === "hide" ? "Hide" : "Show", style, decided_by: "Orb of Unmaking", by_user_edit: state !== "show", reason: "", uncertain: [],
      // ?state=show: decided by a NeverSink step, as the Tiers page names it.
      source: state === "show"
        ? { kind: "neversink", label: "", type_path: ["crafting", "generalgear"], tier: "t1_86", section: "Crafting bases", tier_name: "Tier 1 bases, item level 86+",
            conds: ["not mirrored", "not corrupted", "item level 86+", "white, magic, rare"] }
        : { kind: "edit", label: "Hide: Orb of Unmaking", type_path: [], tier: null, section: null, tier_name: null, only_base: "Orb of Unmaking" } } },
};
mockWindows("overlay");
mockIPC((cmd, args) => {
  if (cmd === "check_item" || cmd === "item_status") return reply;
  // ?jewel=1: a Shimmering Cobalt Jewel; its mod is already in "Jewels with my mods" with one other.
  if (cmd === "jewel_mods") return [{ name: "Shimmering", text: "(6-8)% increased maximum Energy Shield", prefix: true, tags: [] },
    { name: "of Haste", text: "(3-5)% increased Attack Speed", prefix: false, tags: [] }, { name: "Vivid", text: "(5-7)% increased maximum Life", prefix: true, tags: [] }];
  if (cmd === "get_rule" && String((args as { label?: string }).label).includes("Jewels with my mods")) {
    if (String((args as { label?: string }).label).startsWith("Style")) throw "none";
    return ["p", 0, { mods: ["Vivid"], mods_min: 0, style: { font_size: 42, text: [170, 60, 255, 255], border: [170, 60, 255, 255], background: [20, 20, 20, 240] } }];
  }
  if (cmd === "item_price") return [0.82, "chaos"];
  if (cmd === "stack_choices") return [];
  if (cmd === "get_icons") return {};
  // Saved looks (?looks=0 for none yet).
  if (cmd === "list_looks") return params.get("looks") === "0" ? [] : [
    { name: "My chase look", used_by: 2, style: { font_size: 45, text: [255, 60, 60, 255], border: [255, 60, 60, 255], background: [20, 20, 20, 240], beam: { colour: "Red", temp: false }, minimap: null, sound: { kind: "Default" }, label_hidden: false } },
    { name: "Good stuff", used_by: 5, style: { font_size: 42, text: [255, 200, 40, 255], border: [255, 200, 40, 255], background: [20, 20, 20, 240], beam: { colour: "Yellow", temp: false }, minimap: null, sound: { kind: "Default" }, label_hidden: false } },
    { name: "Quiet", used_by: 0, style: { font_size: 30, text: [170, 170, 170, 255], border: null, background: [0, 0, 0, 150], beam: null, minimap: null, sound: { kind: "Off" }, label_hidden: false } },
  ];
  if (cmd === "apply_custom_style") { (window as unknown as { lastApply: unknown }).lastApply = args; return { label: "Style: Orb of Unmaking", warnings: [] }; }
  return null;
}, { shouldMockEvents: true });
document.body.className = "overlay";
const item = params.get("jewel") ? { item_class: "Jewels", rarity: "Magic", name: null, base_type: "Cobalt Jewel", corrupted: false, mirrored: false, unidentified: false, mod_names: ["Shimmering", "of Haste"] }
  : { item_class: "Stackable Currency", rarity: "Normal", name: null, base_type: "Orb of Unmaking", stack_size: 3, corrupted: false, mirrored: false, unidentified: false };
createRoot(document.getElementById("root")!).render(
  <div style={{ width: editor ? 900 : 400, margin: 16 }} className={editor ? "" : "qm"}><ItemPanel start={{ item: item as never, from_screen: !editor, screen_text: "ORB OF UNMAKING", confidence: 1 }} inGame={!editor} onClose={() => {}} /></div>,
);
