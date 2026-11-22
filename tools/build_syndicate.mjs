// Builds src/data/syndicate.json (the Syndicate cheat sheet) from Exile UI's Betrayal data
// (Lailloken/Exile-UI, MIT, data/english/Betrayal.json).
//
//   node tools/build_syndicate.mjs <path to Betrayal.json>
import fs from "node:fs";
import path from "node:path";

const src = process.argv[2];
if (!src) { console.error("usage: node tools/build_syndicate.mjs <Betrayal.json>"); process.exit(1); }
const raw = JSON.parse(fs.readFileSync(src, "utf8"));
const NAMES = { it: "It That Fled" };
const title = (k) => NAMES[k] ?? k.charAt(0).toUpperCase() + k.slice(1);
const members = Object.entries(raw).filter(([k]) => !k.startsWith("_")).map(([k, v]) => ({
  id: k,
  name: title(k),
  // Each job: up to two lines; the first is the reward at that job, "kind:\nwhat" -> { kind, what }.
  jobs: Object.fromEntries(Object.entries(v.rewards).map(([job, list]) => [job, list.map((r) => {
    const [a, b] = r.split("\n");
    return b === undefined ? { kind: "", what: a } : { kind: a.replace(/:$/, ""), what: b };
  })])),
}));
const out = { credit: "Syndicate rewards from Exile UI by Lailloken (MIT).", updated: raw._timestamp ?? "", members };
const dest = path.join(path.dirname(new URL(import.meta.url).pathname), "..", "src", "data", "syndicate.json");
fs.mkdirSync(path.dirname(dest), { recursive: true });
fs.writeFileSync(dest, JSON.stringify(out, null, 1));
console.log(`${members.length} members -> ${dest}`);
