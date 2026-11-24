// What temples (Chronicle of Atzoatl) ask on the trade site, by room: core/data/temples.json, for the farm finder and the
// price check (core/src/temple.rs). The app re-reads the rooms worth something; run this again each league (about ten
// minutes: one request every 8 s, because his own price checks share the trade site's limits).
//
//   node tools/build_temples.mjs [league] [saved.json]       (saved.json: an earlier run's rows, not asked again)
import fs from "node:fs";

const UA = "OAuth poe-filter-editor/0.1 (contact: github repo)";
const SITE = "https://www.pathofexile.com/api/trade";
/** Same as temple.rs MIN_LEVEL: low-level temples sell for next to nothing. */
const MIN_LEVEL = 75;
/** The dearest rooms are also priced two at a time. */
const TOGETHER = 4;
const [league = "Covens", saved] = process.argv.slice(2);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function ask(url, body) {
  for (;;) {
    await sleep(8000);
    const r = await fetch(url, { method: body ? "POST" : "GET", headers: { "User-Agent": UA, "Content-Type": "application/json" }, body: body && JSON.stringify(body) });
    if (r.status === 429) { const s = +(r.headers.get("retry-after") ?? 60) + 5; console.log(`slow down: ${s}s`); await sleep(s * 1000); continue; }
    if (!r.ok) throw new Error(`${url}: ${r.status}`);
    return r.json();
  }
}
const prices = await (await fetch("https://raw.githubusercontent.com/GROBBS-hash/poe-filter-editor/prices/poe1.json")).json();
const rate = { chaos: 1, divine: prices.items.find((i) => i.name === "Divine Orb").value };
const rows = saved ? JSON.parse(fs.readFileSync(saved)).map((r) => ({ label: r.label, stats: r.stats, listed: r.total ?? r.listed, typical: r.typical ?? 0 })) : [];

async function price(stats, label) {
  const old = rows.find((r) => r.label === label);
  if (old) return old;
  const s = await ask(`${SITE}/search/${encodeURIComponent(league)}`, { query: { status: { option: "available" }, type: "Chronicle of Atzoatl",
    stats: [{ type: "and", filters: stats.map((id) => ({ id, value: { option: 1 } })) }],
    filters: { map_filters: { filters: { area_level: { min: MIN_LEVEL } } } } }, sort: { price: "asc" } });
  let list = [];
  if (s.total) {
    const f = await ask(`${SITE}/fetch/${s.result.slice(0, 10).join(",")}?query=${s.id}`);
    list = (f.result ?? []).filter(Boolean).map((r) => (rate[r.listing.price?.currency] ?? NaN) * r.listing.price?.amount).filter((x) => x > 0);
  }
  const row = { label, stats, listed: s.total, typical: list.length ? +list[Math.min(list.length - 1, Math.floor(list.length / 2))].toFixed(1) : 0 };
  rows.push(row);
  console.log(`${label.padEnd(60)} ${String(row.listed).padStart(5)} listed, about ${row.typical}c`);
  return row;
}

const stats = await ask(`${SITE}/data/stats`);
const rooms = stats.result.flatMap((g) => g.entries ?? []).filter((e) => /^Has Room: /.test(e.text) && (/\(Tier 3\)/.test(e.text) || e.id.endsWith("temple_apex")))
  .map((e) => [e.id, e.text.replace("Has Room: ", "")]);
for (const [id, label] of rooms) await price([id], label);
const single = rows.filter((r) => r.stats.length === 1);
const top = single.filter((r) => r.listed > 0).sort((a, b) => b.typical - a.typical).slice(0, TOGETHER);
for (let i = 0; i < top.length; i++) for (let j = i + 1; j < top.length; j++) await price([top[i].stats[0], top[j].stats[0]], `${top[i].label} + ${top[j].label}`);

// The three dearest in one temple.
if (top.length >= 3) await price(top.slice(0, 3).map((r) => r.stats[0]), top.slice(0, 3).map((r) => r.label).join(" + "));

// A plain temple: the middle of every room's asking price.
const asks = single.filter((r) => r.listed > 0).map((r) => r.typical).sort((a, b) => a - b);
const out = { checked: new Date().toISOString().slice(0, 19) + "Z", plain: asks[Math.floor(asks.length / 2)], rooms: rows.filter((r) => r.listed > 0) };
fs.writeFileSync(new URL("../core/data/temples.json", import.meta.url), JSON.stringify(out, null, 1) + "\n");
console.log(`${out.rooms.length} rooms and pairs, plain temple ${out.plain}c`);
