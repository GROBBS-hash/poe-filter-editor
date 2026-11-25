// Atlas node texts -> effects (a number and what it changes), for the atlas builder (core/src/atlas_model.rs).
//   node tools/build_atlas_effects.mjs      writes core/data/atlas_effects.json; run it when public/atlas_poe1.json changes
// Stops with an error when too many lines aren't understood any more (the tree changed): add patterns below.
// Each line -> { mech, lever, op, value, per?, cond?, down? }, then a channel name ("abyss.extra", "altars.upside@eater").
import fs from "node:fs";
const root = new URL("..", import.meta.url);
const T = JSON.parse(fs.readFileSync(new URL("public/atlas_poe1.json", root)));
const WORD = { a: 1, an: 1, one: 1, two: 2, three: 3, four: 4, five: 5, ten: 10 };
const num = s => s in WORD ? WORD[s] : parseFloat(s);

// ---- 1. Text -> clauses. Source lines wrap mid-sentence; keystones pack several sentences in one line.
function clauses(raw) {
  const out = [];
  for (const l of raw.split(/\n| \/ (?!Artisan)/).map(s => s.trim()).filter(Boolean)) {
    const prev = out[out.length - 1];
    const joins = prev && (/^[a-z]/.test(l) || /\b(against|more|of|to|the|for|in|with|when|for each|Final Map Bosses)$/i.test(prev));
    if (joins) out[out.length - 1] += ' ' + l; else out.push(l);
  }
  return out;
}

// ---- 2. Whose line is it (first match wins).
const MECH = [
  ['explicit', /effect of (Explicit|Prefix|Suffix) Modifiers on your Maps/],
  ['harvest', /Sacred F|Lifeforce|Vivid Roots|Primal Roots|Wild Roots/],
  ['essence', /Imprisoned Monster|Essence/],
  ['altars', /Eldritch Altar|Eater of Worlds Altar|Searing Exarch Altar/],
  ['breach', /Breach|Wombgift|Hive|Ailith|Flammable Burrow|Grasping Coffer/],
  ['abyss', /Abyss/], ['legion', /Legion|Timeless Splinter|in stasis/],
  ['blight', /Blight|Tower|Salvaged/], ['betrayal', /Syndicate|Veiled|Allflame/],
  ['incursion', /Incursion|Architect|Stones? of Passage/],
  ['expedition', /Expedition|Logbook|Remnant|Explosive|Runic Monster|Artifacts|Vendor Refresh/],
  ['bestiary', /Beast|Einhar|Hunters' Cunning|Blood Altar/], ['ultimatum', /Ultimatum|7 Ruin|final Round/],
  ['ritual', /Ritual|Favour/], ['delirium', /Delirium/], ['mirage', /Mirage/],
  ['beyond', /Demonic Evocator/], ['sentinel', /Buried Sentinel/], ['wildwood', /Viridian Wildwood/],
  ['settlers', /Ore Deposit|Lost Shipment/], ['mercenaries', /Mercenar/], ['exiles', /Rogue Exile/],
  ['strongboxes', /Strongbox/], ['shrines', /Shrine/], ['torment', /Torment|Possess/],
  ['memory', /Memor/], ['synthesis', /Synthesis/], ['maven', /Maven|additional Bosses summoned|summoned Bosses/],
  ['eldritch', /Eater of Worlds|Searing Exarch/], ['influence', /Influenced by/],
  ['cards', /Divination/], ['gold', /\bGold(en)?\b/], ['threads', /Threads?\b/],
  ['anomalies', /Atlas Anomal/], ['labyrinth', /Labyrinth|Trial of Ascendancy/],
  ['bosses', /Map Boss|Unique Monster|Elder Guardian|Shaper Guardian/],
  ['rares', /Rare Monsters|Mirrored Rare/], ['magics', /Magic (Pack|Monster)|upgraded to Magic/],
  ['mapdrops', /Maps found|Portals to your Maps|Tier 1-15/],
  ['pack', /Pack Size|Monster Packs/], ['rarity', /Rarity of Items found/], ['map', /Maps?\b/],
];
// Lines whose real home is another mechanic than the first word suggests.
const MECH_FIX = [
  [/^Connects to/, 'gateway'],
  [/imprisoned in Essences/, 'exiles'], [/^A Map Boss is granted a random Essence/, 'essence'],
  [/Wisps to Imprisoned|prioritise Imprisoned/, 'harvest'],
  [/Shaped Region/, 'anomalies'], [/Safehouse Atlas Anomaly/, 'betrayal'],
  [/modifier to contain a Mercenary/, 'strongboxes'], [/Allied Mercenaries/, 'mercenaries'],
  [/Mercenaries found in your Maps are accompanied/, 'mercenaries'],
  [/^Wild Rogue Exiles/, 'exiles'], [/random Shrine buff/, 'mercenaries'],
  [/^Map Bosses.*Rogue Exile/, 'exiles'], [/^Map Bosses.*Tormented/, 'torment'],
  [/Map Device has|Threads found|type of Thread|with Threads|additional Thread|drop Threads|for Threads/, 'threads'],
  [/^Divination Cards found|Divination-Touched|stolen Divination/, 'cards'], [/Maven Chisel|Witness/, 'maven'],
  [/Conqueror Map/, 'bosses'], [/Corrupted Rare Maps|1-3 additional random Map|additional random Modifier$|Unallocated Notable/, 'map'],
  [/Golden Volatile|Gold (found|dropped)/, 'gold'],
  [/Eldritch (Ichor|Ember)s? found|Possessing Flame Wraith|Tentacle Mass|encountering The|Implicit Modifier$/, 'eldritch'],
  [/^Unique Monsters.*Ritual Altars/, 'ritual'], [/Shrines in your Maps to be Tormented/, 'shrines'],
  [/Incursion Architects/, 'incursion'], [/Corrupted Growths/, 'settlers'],
];
function mechOf(t) {
  for (const [re, m] of MECH_FIX) if (re.test(t)) return m;
  for (const [m, re] of MECH) if (re.test(t)) return m;
  return null;
}

// ---- 3. What it changes ("lever"), closed vocabulary. Specific rules first, then nouns. [regex, lever]
const LEVER = [
  // map-wide
  [/increased Pack Size in your Maps$/, 'pack'], [/up to 13 Rounds|completed an additional Round/, 'rounds'], [/Cannot have a Boss in the final/, 'boss_chance'],
  [/additional reward per Lane|contain Blighted Maps|Coffers .*Wombgifts|Implicit Modifier$|Jewels .*Veiled Modifier|to have Memory Strands/, 'rewards'],
  [/number of Flammable Burrows/, 'burrows'], [/imprisoned in Essences/, 'essence_link'], [/Essence Modifier from any/, 'boss_mods'],
  [/more Life for each additional Reward/, 'toughness'], [/^Synthesised Monsters/, 'pack'], [/modifier to contain a Mercenary/, 'extra'],
  [/additional Bosses summoned is higher|also apply to these summoned/, 'extra'], [/Cards which drop .* stolen/, 'to_boss'],
  [/effect of Explicit Modifiers on your Maps per different Thread/, 'effect_per_kind'],
  [/effect of Explicit Modifiers on your Maps per Explicit/, 'effect_per_mod'],
  [/effect of Prefix Modifiers/, 'effect_prefix'], [/effect of Suffix Modifiers/, 'effect_suffix'],
  [/effect of Explicit Modifiers on your Maps/, 'effect'],
  [/additional random (Map )?Modifiers?|modified unpredictably/, 'extra_map_mods'],
  [/Packs consisting of difficult and rewarding/, 'hard_packs'], [/Unallocated Notable/, 'random_notables'],
  [/Influenced by .* increased Pack Size/, 'pack'], [/increased Magic Pack Size/, 'magic_pack'],
  [/upgraded to Magic/, 'magic_packs'], [/additional Packs with Mirrored Rare/, 'mirrored_rares'],
  [/Rare Monsters in each of your Maps are Duplicated/, 'dup_rares'],
  [/^\d+% increased number of Magic Monsters/, 'magic'], [/^\d+% increased number of Rare Monsters/, 'rare'],
  [/^Rare Monsters in your Maps have an additional Modifier/, 'rare_mods'],
  [/^Unique Monsters in your Maps have .*additional Monster Modifier/, 'boss_mods'],
  [/Rare Monsters in each of your Maps are Possessed/, 'possess_rares'],
  [/^Monsters in your Maps (deal|have) \d+% (more|less) (Damage|Life)/, 'toughness'],
  [/drop Threads per Monster Modifier/, 'found_per_rare_mod'],
  [/Unique Monsters in your Maps have 200% increased chance to drop Threads/, 'found_unique'],
  [/Threads found in your Maps$/, 'found'], [/drop an additional Thread/, 'boss_extra'],
  [/not to consume Threads|2 additional Threads/, 'thread_keep'], [/only apply one type of Thread/, 'thread_limit'],
  [/converted to Gold/, 'to_gold'],
  [/Maps found in your Maps for each/, 'found_per_mod'], [/^\d+% increased Maps found/, 'found'],
  [/^Tier 1-15 Maps found/, 'tier_up'], [/Portals to your Maps/, 'portal'], [/additional Map at a tier higher/, 'boss_extra'],
  [/Rarity of Items found in your Maps$/, 'rarity'], [/Memory Influence$/, 'map_influence'],
  [/Golden Volatile|stack size of Gold|Gold dropped/, 'rewards'],
  [/^Connects to/, 'gateway'],
  // altars
  [/always have one choice affecting/, 'choice'], [/have an additional Upside$/, 'extra_upside'],
  [/Effect of Upsides/, 'upside'], [/Downside/, 'downside'], [/chance to (find|spawn a) .*Altar/, 'chance'],
  [/double progress/, 'progress'],
  // one-off prizes
  [/Evolving Shrine|Exceptional$|Tormented Procurator|Empyrean|Calcified Map|Scrying Orb|Safehouse Atlas|Lost Shipments|Conqueror Map|Guardian Map|Synthesis Map|Blood-filled Vessel|Oversoul|Vaal Vessels|chance to drop a Talisman|Enshrouding Crystal|Improved Offering|Maven Chisels|Possessing Flame Wraith|Touching Tentacle Mass|as Allflames/, 'jackpot'],
  // steering: which variety
  [/Breach Encounters in your Maps are always/, 'mode'],
  [/(of type|cannot be of type|certain (Essences|Sacred Flora|Tormented)|to be from|of class|led by|w+ aligned|include an? (Eternal|Templar|Vaal|Karui|Maraketh)|Wombgifts found in your Maps to be|Ore Deposits found in your Maps to be|do not have Suffix|chance to reward|Rewards in your Maps have 80%|require you to|contain an additional Red Beast from|lead towards)/, 'steer'],
  // wisps / possession
  [/Wisps/, 'wisps'], [/(are|is|be) (Possessed|Tormented)|Possessed by a/, 'possess'],
  // harvest, ritual, essence specifics
  [/Sacred Flora in your Maps have \d+% increased effect/, 'flora_effect'],
  [/Strongboxes in your Maps have 50% increased effect/, 'box_effect'],
  [/Quantity of Lifeforce|random Crops/, 'rewards'], [/Sacred Fauna .*(Tier 3|be a Boss)|cannot be Normal/, 'tier_up'],
  [/Lifeforce dropped .* Duplicated|Fauna in your Maps have .*duplicated/, 'dup'],
  [/allow rerolling|has no Cost 1|Rerolling Favours|reappear|Deferring Favours/, 'rerolls'],
  [/randomly cost/, 'spread'], [/Special Rewards|Itemised Corpse/, 'special'],
  [/penalty to Tribute|increased Tribute|more Tribute/, 'rewards'], [/increased Favours/, 'rewards'],
  [/Imprisoned Monsters in your Maps have .*additional Essence|have an additional Essence/, 'rewards'],
  [/Essences found in your Maps have .* tier higher|highest possible tier|replaces all Essences|Corrupted an additional time/, 'tier_up'],
  [/Essence Modifier from any/, 'rule'],
  // encounters
  [/Experience/, 'xp'],
  [/Breach Bosses .* Duplicated/, 'boss_dup'], [/Fortresses$/, 'fortress'], [/chance to lead to an Abyssal Depths/, 'depths'],
  [/include a General|both Generals/, 'generals'], [/final Round to include a Boss/, 'boss_chance'],
  [/have additional waves|additional waves/, 'extra'], [/copy of Beasts/, 'dup'],
  [/appear in Pairs|openable again|duplicated|Duplicated|doubled Ore|not be consumed when/, 'dup'],
  [/not lose Rank|additional Rank|Upgrade Tier/, 'ranks'],
  [/contain (multiple|two additional|three additional|ten additional)|contain an additional (Unstable|Red Beast|Yellow)|\d additional (Red|Yellow) Beast|contain \d additional Pits|contain an additional Pit|roll number of Pits|replaced with Red Beasts|more likely to contain multiple Lanes|\+1 Remnants|Runic Monster Markers|always have four/, 'extra'],
  [/20 additional Rogue Exiles/, 'extra'],
  [/(Surpassing Chance|chance) (to contain|to be haunted|to be inhabited)|chance to contain (a|an|the) |Atlas Anomalies (to|can|manifest)|contain Memory Tears|additional Apparition/, 'spawn'],
  [/Witnessing|additional Atlas Bosses|summoned Bosses/, 'extra'],
  // who spawns
  [/Pit will have an additional Rare|guarded by 5 additional Rare|spawn 2 additional Rare|dig up a Rare/, 'rare_add'],
  [/increased (number of )?Rare [Mm]onsters|spawn Rare Packs/, 'rare'], [/at least (Magic|one Magic)|increased Magic Monsters|spawn Magic Packs/, 'magic'],
  [/guarded by (an|2) additional|additional Packs?\b|additional pack|accompanied by|surrounded by|spawn packs|increased number of Packs/, 'packs'],
  [/Pack Size$|increased (Non-Unique )?Monsters|number of Monsters$|Spawned Monsters|Monsters per fed Soul/, 'pack'],
  [/additional Blight Bosses|additional Sergeants/, 'extra'],
  [/skills created by Ailith|Ailith's/, 'ailith'],
  // what it pays
  [/Suffix Modifier$|Prefix Modifier$/, 'remnant_mods'], [/Difficulty and Reward|increased Difficulty|Bestial Rage|Hunters' Cunning/, 'diff_reward'],
  [/more likely to be rarer|less common varieties|more likely to be less|are Lucky|be Lucky|at least Rare|Item Tiers|Corrupted|Unique$|are Unique|Infamous$|grant Smelted|always have 4 Implicit|replaced by Inscribed|Inscribed Ultimatums$|start a Tier higher|Fortresses/, 'tier_up'],
  [/Item Rarity|Rarity of Items/, 'item_rarity'],
  [/Abyssal Crystal/, 'extra'],
  [/Modifiers? (affecting|for each|to Monsters)|additional (Monster |Bestiary )?Modifier/, 'extra_mods'],
  [/Reward|War Hoard|Quantity of|quantity of|Stack Size|increased Ore|drop (an additional|all|1 of|\d+% more|10 of)|drops 10 of|Wombgift|Divination Cards found|Bargain|Rucksacks|Blighted Maps|additional Red Beast|Voltaxic|Abyssal Weavings|Hiveblood|Gold if|additional Item/, 'rewards'],
  [/be stolen|stolen by/, 'rule'],
  // how it feels: time, danger, power
  [/be set free/, 'dup'],
  [/more Damage against|Einhar (deals|has|remains)|Shrine Buffs|Shrine buff|Buffs granted|Shrine Effect|Empowering|salvaged|Salvaged|Damage of skills|while in Stasis|of Life missing|take 10% increased Damage|reduced Damage$|50% less Damage|Towers? .*more Damage|increased Life$/, 'power'],
  [/faster|Duration|duration|Radius|radius|Range|Timer|Schism|seconds|slower|Cost\b|Time gained|Explosives?\b|Stone Circles|reduced number of Enemies/, 'speed'],
  [/more Life|increased Life|Life for each|Toughness|Damage|Onslaught|Cooldown|Auras|Support|Safeguards|Ruin|require 100% increased Gold/, 'toughness'],
  [/Possess|Touched/, 'possess'], [/encounter begins|begin once/, 'speed'],
  [/cannot|always|can |are |instead|have |is |from|stasis/i, 'rule'],
];

// ---- 4. Conditions and sides.
function conds(t) {
  const c = [];
  const add = (re, k) => { const m = t.match(re); if (m) c.push(typeof k === 'function' ? k(m) : k); };
  add(/\(Tier (\d+)\+\)/, m => 'tier>=' + m[1]); add(/[Tt]ier 16 Maps/, 'tier>=16'); add(/level (\d+)\+/, m => 'level>=' + m[1]);
  add(/Corrupted (Rare )?Maps/, 'map:corrupted'); add(/Influenced by Conquerors/, 'influence:conquerors');
  add(/nfluenced by The Shaper/, 'influence:shaper'); add(/nfluenced by The Elder/, 'influence:elder');
  add(/Eater of Worlds/, 'eldritch:eater'); add(/Searing Exarch/, 'eldritch:exarch');
  add(/Unstable Breach|Stabilised|Breach already opened/, 'breach:unstable'); add(/\bHive\b|Hives|Ailith|Burrow|Fortress|Grasping Coffer/, 'breach:hive');
  add(/Final Map Boss/, 'kill:finalboss'); add(/Witness/, 'maven:witness');
  add(/fewer than 50|fewer monsters remaining/, 'cleared:almost'); add(/in Synthesis Maps/, 'map:synthesis');
  return [...new Set(c)];
}
// ---- 5. Numbers: what the line is worth as one expected number.
const NUMW = '(\\d+(?:\\.\\d+)?|an?|one|two|three|four|ten)';
const PER = /(?:\bper|for each|for every|\beach) (different Thread|Explicit Modifier|Monster Modifier|prior Pit|fed Soul|completed Ritual|Incursion completed|Unstable Breach already opened|Breach already opened|Remnant detonated|Flammable Burrow destroyed|Essence Modifier|stolen Divination Card|1% Map Pack Size|additional Reward|Lane|Round)/i;
function value(t) {
  let m;
  const per = (t.match(PER) || [])[1];
  const base = per ? { per } : {};
  if (m = t.match(new RegExp('(\\d+(?:\\.\\d+)?)% (Surpassing )?(?:additional )?chance.*?\\b(?:contain|have|with|gain|by|drop|add|spawn|for|at the Blood)\\b ' + NUMW + ' additional', 'i')))
    return { ...base, op: 'expect', value: +(+m[1] / 100 * num(m[3].toLowerCase())).toFixed(4), chance: +m[1], each: num(m[3].toLowerCase()), ...(m[2] ? { surpassing: true } : {}) };
  if (m = t.match(/\+?(\d+(?:\.\d+)?)% Surpassing Chance/i)) return { ...base, op: 'surpass', value: +m[1] };
  if (m = t.match(/between (\d+)% less and (\d+)% more/)) return { ...base, op: 'spread', value: (m[2] - m[1]) / 2, lo: -m[1], hi: +m[2] };
  if (m = t.match(/(\d+(?:\.\d+)?)% (increased|reduced|more|less)/)) return { ...base, op: /incr|red/.test(m[2]) ? 'inc' : 'more', value: (/reduced|less/.test(m[2]) ? -1 : 1) * m[1] };
  if (m = t.match(/(\d+)% (slower|faster)/)) return { ...base, op: 'inc', value: (m[2] === 'slower' ? -1 : 1) * m[1] };
  if (m = t.match(/(\d+)% sooner/)) return { ...base, op: 'inc', value: +m[1] };
  if (m = t.match(/\+?(\d+(?:\.\d+)?)%\s+(?:additional )?chance/)) return { ...base, op: 'chance', value: +m[1] };
  if (/twice|Doubled|doubled/.test(t)) return { ...base, op: 'more', value: 100 };
  if (/Halved/.test(t)) return { ...base, op: 'more', value: -50 };
  if (m = t.match(/up to (\d+) additional|summoning 1 to (\d+) additional|with 1-(\d+) additional|have 1-(\d+) additional/)) { const hi = +(m[1] || m[2] || m[3] || m[4]); return { ...base, op: 'add', value: (1 + hi) / 2, hi }; }
  if (m = t.match(new RegExp('\\+?' + NUMW + ' additional', 'i'))) return { ...base, op: 'add', value: num(m[1].toLowerCase()) };
  if (m = t.match(/equal to (\d+)%|additional (\d+)% of|(\d+)% of/)) return { ...base, op: 'inc', value: +(m[1] || m[2] || m[3]) };
  if (m = t.match(/\+(\d+) /)) return { ...base, op: 'add', value: +m[1] };
  if (/always have four/.test(t)) return { ...base, op: 'set', value: 4 };
  if (m = t.match(/(?:up to|drops|At least|last up to|reaching|contained in) (\d+)\b/i)) return { ...base, op: 'count', value: +m[1] };
  if (m = t.match(/^(\d+) (Rare|Monster)|with (\d+) additional/)) return { ...base, op: 'count', value: +(m[1] || m[3]) };
  if (m = t.match(/\b(accompanied by two|drop 1 of|is one|from two|spawn packs|always have 4)\b/)) return { ...base, op: 'count', value: /two/.test(m[1]) ? 2 : /4/.test(m[1]) ? 4 : 1 };
  return { ...base, op: 'flag', value: 1 };
}
// Makes the map harder or takes something away (the "cost" of a node).
const DOWN = /additional Downside|Effect of Downsides|deal \d+% (more|increased) Damage|^Monsters .*more Life|more Life$|increased Life$|increased Toughness|Toughness (of|per)|Toughness for|deals 1% increased Damage|cannot (choose|Possess Monsters|have a Boss)|players cannot|only apply one|converted to Gold|Explosives is one|Fail on|take 75% less|reduced Radius|require 100% increased|increased Difficulty|Skills? .*Ruin|apply Ruin|Safeguards|Mercenaries .*Onslaught|increased Cooldown Recovery Rate$(?<!Einhar.*)|Beasts .*Onslaught/i;

function parseLine(raw) {
  return clauses(raw).map(t => {
    const mech = mechOf(t);
    let lever = null;
    for (const [re, l] of LEVER) if (re.test(t)) { lever = l; break; }
    const e = { mech, lever, ...value(t) };
    if (/reduced (penalty to Tribute|Cost)|costs? \d+% reduced/.test(t)) e.value = Math.abs(e.value); // cheaper / less penalty is good
    if (/Ultimatum Modifier is applied/.test(t)) { e.mech = 'ultimatum'; delete e.per; }
    const c = conds(t); if (c.length) e.cond = c;
    if (DOWN.test(t) && !/^(Allied Mercenaries|Blight Towers)/.test(t)) e.down = true;
    e.text = t;
    return e;
  });
}

// ---- 6. Whole tree. Choice notables whose own text is the start of a sentence: the option finishes it.
const OPEN = /(to be|of type|from|affecting|certain Essences|certain Sacred Flora|certain Tormented Spirits|of class|are always)$/;
const nodes = {}; let lines = 0;
const stat = { full: 0, flag: 0, vague: 0, none: 0 }; const left = [], vocab = {}, tmpl = {}, uniq = { all: new Set(), bad: new Set() };
function note(e) {
  lines++; uniq.all.add(e.text);
  const k = !e.mech || !e.lever ? 'none' : e.lever === 'rule' ? 'vague' : e.op === 'flag' ? 'flag' : 'full';
  stat[k]++; if (k === 'none') { left.push(e.text); uniq.bad.add(e.text); }
  vocab[e.mech + '.' + e.lever] = (vocab[e.mech + '.' + e.lever] || 0) + 1;
  (tmpl[`${e.mech}.${e.lever} ${e.op}`] ??= new Set()).add(`${e.value}${e.per ? ' per ' + e.per : ''}${e.cond ? ' [' + e.cond + ']' : ''}${e.down ? ' DOWN' : ''} :: ${e.text}`);
}
for (const [id, n] of Object.entries(T.nodes)) {
  const opts = T.choices[id];
  const stats = n[4].filter(s => !/^Select a type of content/.test(s));
  const o = { kind: n[2], name: n[3] };
  const open = opts ? stats.flatMap(clauses).filter(s => OPEN.test(s)) : [];
  o.effects = stats.flatMap(clauses).filter(s => !open.includes(s)).flatMap(parseLine);
  o.effects.forEach(note);
  if (opts) {
    o.options = Object.entries(opts).map(([oid, t]) => {
      const texts = open.length ? open.map(s => s + ' ' + t) : clauses(t);
      const ef = texts.flatMap(parseLine); ef.forEach(note);
      return { id: +oid, text: t, effects: ef };
    });
  }
  nodes[id] = o;
}

// ---- 7. Lines -> channels the model reads (core/data/atlas_model.json names the same channels).
// Lines two nodes word alike but that do different things get a channel of their own (first match wins).
const SPLIT = [
  [/Ritual Altars.*Wildwood Wisps/, "ritual.wisps"], [/Final Map Boss.*Wildwood Wisps/, "bosses.wisps"], [/Wildwood Wisps.*Imprisoned/, "essence.wisps"],
  [/count as also Witnessing/, "maven.witness"], [/additional seconds to collapse/, "breach.collapse"], [/per fed Soul/, "abyss.chasm"],
  [/Quantity of Wombgifts/, "breach.wombgifts"], [/Quantity of Hiveblood/, "breach.hiveblood"], [/Abysses in your Maps roll number of Pits twice/, "abyss.pits_twice"],
  [/Abyssal Crystal/, "abyss.crystal"], [/quantity of Abyss Jewels/, "abyss.jewels"], [/Abyssal Weavings/, "abyss.weavings"], [/twice as likely to have Rewards/, "abyss.double_rewards"],
  [/War Hoard/, "legion.war_hoard"], [/Legion Sergeants .* Rewards/, "legion.sergeants"], [/Random Reward/, "legion.chests"], [/spread their Rewards/, "legion.chest_spread"], [/additional Sergeants/, "legion.more_sergeants"],
  [/random Crops/, "harvest.crops"], [/cannot be Normal Rarity/, "harvest.flora_rare"], [/chance to be a Boss/, "harvest.specimen"], [/spread Wildwood Wisps to Rare and Unique/, "harvest.wisps_rares"], [/Wildwood Wisps/, "harvest.wisp_count"],
  [/penalty to Tribute/, "ritual.penalty"], [/increased Favours/, "ritual.favours"], [/Ritual Splinters/, "ritual.splinters"], [/Unique Monsters slain at Ritual/, "ritual.bosstribute"], [/Deferr/, "ritual.defer"], [/Rerolling Favours/, "ritual.reroll_cost"], [/always have four Ritual/, "ritual.four"],
  [/Rarity of Items from Unique Monsters in your Maps per/, "bosses.rarity_per_pack"], [/additional Yellow Beast/, "bestiary.yellow"], [/replaced with Red/, "bestiary.convert"], [/additional Red Beast from/, "bestiary.extra"], [/copy of Beasts/, "bestiary.copy"], [/Hunters' Cunning/, "bestiary.cunning"],
  [/two Rogue Exile Bodyguards/, "exiles.bodyguards"], [/multiple Ore/, "settlers.multi"], [/replaced by Lost Shipments/, "settlers.shipment"], [/Verisium/, "settlers.verisium"], [/Petrified Amber|Orichalcum|Crimson/, "settlers.slow_ore"],
  [/Guardian Map/, "bosses.guardian_maps"], [/Conqueror Map/, "bosses.conqueror_maps"], [/Synthesis Map to drop/, "bosses.synthesis_maps"], [/Maven Chisels/, "maven.chisels"],
  [/Apparition Shrine/, "shrines.apparition"], [/random additional Shrine Effect/, "shrines.extra_effect"], [/Shrine Buffs on players/, "shrines.buff"], [/Evolving Shrine/, "shrines.evolving"],
  [/two random other Maps/, "cards.paths"], [/Scrying Orb/, "cards.scrying"], [/Gold dropped by Unique/, "gold.unique"], [/Golden Volatile/, "gold.volatile"],
  [/at the highest possible tier/, "essence.top_tier"], [/can be Corrupted an additional time/, "essence.corrupt_again"], [/3 additional Essences/, "essence.lattice"], [/Exiles .*imprisoned in Essences/, "essence.from_exiles"], [/Calcified/, "essence.calcified"], [/replaces all Essences/, "essence.commensurate"],
  [/manifest a Safehouse/, "betrayal.safehouse"], [/additional Rank/, "betrayal.rank_up"], [/not lose Rank/, "betrayal.keep_rank"],
  [/Incursions always have four|always have four Incursions/, "incursion.four"], [/surviving Architect/, "incursion.upgrade"],
  [/Oversoul/, "ultimatum.oversoul"], [/completed an additional Round/, "ultimatum.free_round"], [/Inscribed/, "ultimatum.inscribed"],
  [/Explosives is one/, "expedition.one_blast"],
];
const channel = (e) => {
  let c = `${e.mech}.${e.lever}`;
  for (const [r, n] of SPLIT) if (r.test(e.text)) { c = n; break; }
  return c;
};
const compact = (e) => {
  if (e.mech === "gateway") return null;
  const cond = e.cond ?? [], need = cond.filter((c) => !/^breach:|^kill:|^cleared:/.test(c));
  let c, v = ["inc", "more", "chance", "surpass"].includes(e.op) ? e.value / 100 : e.value, per = e.per;
  if (e.mech === "threads" && e.lever === "steer") { const m = e.text.match(/type ([A-Za-z]+)/); c = `steer:${/cannot/.test(e.text) ? "-" : "+"}${m[1]}`; }
  else if (e.mech === "altars" && e.lever === "choice") c = "altars.pick_" + (/Players/.test(e.text) ? "players" : /Bosses/.test(e.text) ? "bosses" : "monsters");
  else if (per === "different Thread") { c = "explicit.per_kind"; per = undefined; }
  else if (per === "Explicit Modifier") { c = e.mech === "mapdrops" ? "mapdrops.found_per_mod" : "explicit.per_mod"; per = undefined; }
  else if (per === "Monster Modifier") { c = "threads.per_mod"; per = undefined; }
  else if (/gain 2 additional Threads/.test(e.text)) c = "threads.plus2";
  else if ((e.mech === "maven" && e.lever === "extra" && e.op === "flag") || (e.mech === "map" && e.lever === "extra_map_mods" && e.op === "flag")) return null;
  else {
    c = channel(e);
    const side = cond.find((x) => /^breach:/.test(x));
    if (side && c.startsWith("breach.")) c += "@" + side.split(":")[1];
    if (e.mech === "abyss" && e.lever === "extra" && e.op === "more") v = 1;
  }
  const o = { c, v: +v.toFixed(5) };
  if (per) o.per = per;
  if (need.length) o.need = need;
  if (e.down) o.down = true;
  return o;
};
const out = {};
for (const [id, n] of Object.entries(nodes)) {
  const e = n.effects.map(compact).filter(Boolean), o = n.options?.map((x) => ({ id: x.id, e: x.effects.map(compact).filter(Boolean) }));
  if (e.length || o) out[id] = o ? { e, o } : { e };
}
const bad = stat.none + stat.vague;
console.log(`${lines} lines: ${stat.full} with a number, ${stat.flag} yes/no, ${bad} not understood`);
if (bad / lines > 0.01) { console.error(left.join("; ")); process.exit(1); }
const json = JSON.stringify({ version: T.version, nodes: out }).replace(/("[0-9]+":[{]"e")/g, String.fromCharCode(10) + "$1");
fs.writeFileSync(new URL("core/data/atlas_effects.json", root), json + String.fromCharCode(10));
