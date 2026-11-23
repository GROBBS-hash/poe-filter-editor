//! Reads and writes NeverSink-style loot filters without losing anything.
//! Every original line (including its \r\n ending and any extra tag tokens
//! like `%H8` or `!uniques_a`) is kept verbatim unless we deliberately change it.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub visibility: String,     // Show | Hide | Minimal
    pub type_path: Vec<String>, // e.g. ["uniques","replicas"]
    pub tier: Option<String>,   // e.g. "t1"
    pub header: String,         // original header line, ending included
    pub lines: Vec<String>,     // original body lines, endings included
    pub start_line: usize,
    #[serde(skip)]
    newline: String,
}

impl Block {
    fn base_line_index(&self) -> Option<usize> {
        self.lines.iter().position(|l| l.trim_start().starts_with("BaseType"))
    }

    pub fn base_types(&self) -> Vec<String> {
        self.base_line_index()
            .map(|i| quoted_values(&self.lines[i]))
            .unwrap_or_default()
    }

    /// Replaces the BaseType list, keeping the original indentation,
    /// operator and line ending. An emptied list comments the block out,
    /// because the game rejects `BaseType ==` with no values.
    pub fn set_base_types(&mut self, bases: &[String]) {
        let Some(i) = self.base_line_index() else { return };
        let line = &self.lines[i];
        let indent: String = line.chars().take_while(|c| c.is_whitespace() && *c != '\r' && *c != '\n').collect();
        let body = line.trim();
        let op = if body.starts_with("BaseType ==") { "BaseType ==" } else { "BaseType" };
        if bases.is_empty() {
            self.lines[i] = format!("{indent}# {op} (emptied by editor){}", self.newline);
        } else {
            let list = bases.iter().map(|b| format!("\"{b}\"")).collect::<Vec<_>>().join(" ");
            self.lines[i] = format!("{indent}{op} {list}{}", self.newline);
        }
    }

    pub fn render(&self) -> String {
        if self.disabled_empty() {
            // Comment out the whole block so the game skips it.
            let mut out = format!("# {}", self.header);
            for l in &self.lines {
                out.push_str("# ");
                out.push_str(l);
            }
            return out;
        }
        let mut out = self.header.clone();
        for l in &self.lines {
            out.push_str(l);
        }
        out
    }

    /// Show / Hide the whole block (keeps the rest of the header line).
    pub fn set_visibility(&mut self, show: bool) {
        let kw = if show { "Show" } else { "Hide" };
        let lead: String = self.header.chars().take_while(|c| c.is_whitespace()).collect();
        let rest = self.header.trim_start();
        let rest = rest.strip_prefix(self.visibility.as_str()).unwrap_or(rest);
        self.header = format!("{lead}{kw}{rest}");
        self.visibility = kw.to_string();
    }

    /// Sets "StackSize >= n" (replacing any StackSize line, or adding one).
    pub fn set_min_stack(&mut self, n: u32) {
        self.set_min("StackSize", n);
    }

    /// Sets "<key> >= n", e.g. "UnidentifiedItemTier >= 4" (replacing that line, or adding one).
    pub fn set_min(&mut self, key: &str, n: u32) {
        let indent: String = self.lines.first().map(|l| l.chars().take_while(|c| *c == ' ' || *c == '\t').collect()).unwrap_or_else(|| "\t".into());
        let line = format!("{indent}{key} >= {n}{}", self.newline);
        match self.lines.iter().position(|l| l.trim_start().split_whitespace().next() == Some(key)) {
            Some(i) => self.lines[i] = line,
            None => self.lines.insert(0, line),
        }
    }

    /// Changes this step's conditions: each key's line(s) replaced by the new value, or removed when it's "".
    pub fn set_conds(&mut self, edits: &crate::conds::CondEdits) {
        let indent: String = self.lines.first().map(|l| l.chars().take_while(|c| *c == ' ' || *c == '\t').collect()).unwrap_or_else(|| "\t".into());
        for (key, value) in edits {
            let at = self.lines.iter().position(|l| crate::conds::line_is(l, key)).unwrap_or(0);
            self.lines.retain(|l| !crate::conds::line_is(l, key));
            let at = at.min(self.lines.len());
            let new: Vec<String> = crate::conds::lines_for(key, value).into_iter().map(|l| format!("{indent}{l}{}", self.newline)).collect();
            self.lines.splice(at..at, new);
        }
    }

    /// Replaces the block's look and sound lines with `actions`.
    pub fn replace_style(&mut self, actions: &[String]) {
        const STYLE: [&str; 14] = ["SetFontSize", "SetTextColor", "SetBorderColor", "SetBackgroundColor", "PlayAlertSound",
            "PlayAlertSoundPositional", "CustomAlertSound", "CustomAlertSoundOptional", "DisableDropSound", "EnableDropSound",
            "DisableDropSoundIfAlertSound", "EnableDropSoundIfAlertSound", "PlayEffect", "MinimapIcon"];
        let key = |l: &str| l.trim_start().split_whitespace().next().unwrap_or("").to_string();
        let indent: String = self.lines.first().map(|l| l.chars().take_while(|c| *c == ' ' || *c == '\t').collect()).unwrap_or_else(|| "\t".into());
        self.lines.retain(|l| !STYLE.contains(&key(l).as_str()));
        let at = self.lines.iter().position(|l| key(l) == "Continue").unwrap_or(self.lines.len());
        let new: Vec<String> = actions.iter().map(|a| format!("{indent}{a}{}", self.newline)).collect();
        self.lines.splice(at..at, new);
    }

    fn disabled_empty(&self) -> bool {
        self.lines.iter().any(|l| l.contains("(emptied by editor)"))
    }
}

fn quoted_values(line: &str) -> Vec<String> {
    line.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

fn parse_tag(header: &str) -> (Vec<String>, Option<String>) {
    let mut path = Vec::new();
    let mut tier = None;
    for word in header.split_whitespace() {
        if let Some(p) = word.strip_prefix("$type->") {
            path = p.split("->").map(str::to_string).collect();
        } else if let Some(t) = word.strip_prefix("$tier->") {
            tier = Some(t.to_string());
        }
    }
    (path, tier)
}

pub enum Segment {
    Raw(String),
    Block(Block),
}

fn block_keyword(t: &str) -> Option<&'static str> {
    ["Show", "Hide", "Minimal"].into_iter().find(|k| {
        t.starts_with(k)
            && t[k.len()..].chars().next().map_or(true, |c| c.is_whitespace() || c == '#')
    })
}

/// Switches on NeverSink's commented-out blocks of one `$type` (e.g. `maps->implicitmod`, which he ships
/// as "#Show" lines), in place, so they work like his other rules. Returns how many were switched on.
/// Is this tier in the filter but switched off ("#Show # $type->… $tier->…")? NeverSink does that at stricter
/// levels (Uber Strict turns off the levelling rares, stacked fragments…), so an edit made on a looser filter has
/// nothing to change there, but the tier isn't gone.
pub fn has_commented(segs: &[Segment], type_path: &[String], tier: &str) -> bool {
    segs.iter().any(|s| {
        let Segment::Raw(raw) = s else { return false };
        raw.lines().any(|line| {
            let Some(u) = line.trim_start().strip_prefix('#') else { return false };
            if block_keyword(u).is_none() { return false; }
            let (path, t) = parse_tag(u.trim_end());
            path == type_path && t.as_deref() == Some(tier)
        })
    })
}

pub fn enable_commented(segs: &mut Vec<Segment>, type_path: &[&str]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < segs.len() {
        let Segment::Raw(raw) = &segs[i] else { i += 1; continue };
        let mut out = String::with_capacity(raw.len());
        let mut inside = false;
        let mut changed = false;
        for line in raw.split_inclusive('\n') {
            let t = line.trim_start();
            let uncommented = t.strip_prefix('#');
            let header = uncommented.filter(|u| block_keyword(u).is_some()).map(|u| parse_tag(u.trim_end()).0);
            if let Some(path) = header {
                inside = path.iter().map(String::as_str).eq(type_path.iter().copied());
                if inside { out.push_str(uncommented.unwrap()); count += 1; changed = true; continue; }
            } else if inside {
                // The block's own lines are "#\tCondition": anything else (a blank line, a "#---" note) ends it.
                match uncommented {
                    Some(u) if u.starts_with(['\t', ' ']) && !u.trim().is_empty() => { out.push_str(u); continue; }
                    _ => inside = false,
                }
            }
            out.push_str(line);
        }
        if changed {
            let parsed = parse_filter(&out);
            let n = parsed.len();
            segs.splice(i..=i, parsed);
            i += n;
        } else {
            i += 1;
        }
    }
    count
}

pub fn parse_filter(text: &str) -> Vec<Segment> {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" }.to_string();
    let mut segs = Vec::new();
    let mut raw = String::new();
    let mut cur: Option<Block> = None;

    for (i, line) in text.split_inclusive('\n').enumerate() {
        let t = line.trim_start();
        let content = line.trim_end_matches(['\r', '\n']);

        if let Some(kw) = block_keyword(t) {
            if let Some(b) = cur.take() { segs.push(Segment::Block(b)); }
            if !raw.is_empty() { segs.push(Segment::Raw(std::mem::take(&mut raw))); }
            let (type_path, tier) = parse_tag(content);
            cur = Some(Block {
                visibility: kw.to_string(),
                type_path,
                tier,
                header: line.to_string(),
                lines: vec![],
                start_line: i,
                newline: newline.clone(),
            });
            continue;
        }

        let ends_block = content.trim().is_empty()
            || (t.starts_with('#') && !line.starts_with([' ', '\t']));
        match cur.as_mut() {
            Some(b) if !ends_block => b.lines.push(line.to_string()),
            Some(_) => {
                segs.push(Segment::Block(cur.take().unwrap()));
                raw.push_str(line);
            }
            None => raw.push_str(line),
        }
    }
    if let Some(b) = cur { segs.push(Segment::Block(b)); }
    if !raw.is_empty() { segs.push(Segment::Raw(raw)); }
    segs
}

pub fn render_filter(segs: &[Segment]) -> String {
    segs.iter()
        .map(|s| match s {
            Segment::Raw(r) => r.clone(),
            Segment::Block(b) => b.render(),
        })
        .collect()
}

/// Moves a base type into `to_tier` within the section `type_path`,
/// removing it from any other tier of that same section.
/// Returns false if the target tier doesn't exist.
pub fn move_base(segs: &mut [Segment], type_path: &[&str], base: &str, to_tier: &str) -> bool {
    let same_path = |b: &Block| b.type_path.iter().map(String::as_str).eq(type_path.iter().copied());
    let target_exists = segs.iter().any(|s| matches!(s, Segment::Block(b)
        if same_path(b) && b.tier.as_deref() == Some(to_tier) && !b.base_types().is_empty()));
    if !target_exists { return false; }

    let mut placed = false;
    for s in segs.iter_mut() {
        let Segment::Block(b) = s else { continue };
        if !same_path(b) { continue; }
        let mut bases = b.base_types();
        let has = bases.iter().any(|x| x == base);
        if b.tier.as_deref() == Some(to_tier) {
            if !placed && !has { bases.push(base.to_string()); b.set_base_types(&bases); }
            placed = true;
        } else if has {
            bases.retain(|x| x != base);
            b.set_base_types(&bases);
        }
    }
    placed
}

#[cfg(test)]
mod commented_tests {
    use super::*;

    #[test]
    fn switched_off_tier_is_not_gone() {
        let text = "Show # %D4 $type->leveling->rare->armours $tier->general !x\n\tRarity Rare\n\n\
#Show # %DS3 $type->fragments $tier->t3stackedex !fragments_b\n#\tStackSize >= 3\n#\tClass == \"Map Fragments\"\n\n";
        let segs = parse_filter(text);
        let p = |s: &str| s.split("->").map(String::from).collect::<Vec<_>>();
        assert!(has_commented(&segs, &p("fragments"), "t3stackedex"));
        assert!(!has_commented(&segs, &p("fragments"), "t3"));
        // A switched-on block isn't "commented".
        assert!(!has_commented(&segs, &p("leveling->rare->armours"), "general"));
    }

    /// His real Uber Strict file (dev only): every tier the 23 Nov message called "gone" is switched off there.
    #[test]
    #[ignore]
    fn his_uber_strict() {
        let dir = std::env::var("APPDATA").unwrap() + "/com.grobbs.poefiltereditor/neversink/poe1/NeverSink's filter - 5-UBER-STRICT.filter";
        let segs = parse_filter(&std::fs::read_to_string(dir).unwrap());
        let p = |s: &str| s.split("->").map(String::from).collect::<Vec<_>>();
        for (path, tier) in [("fragments", "t3stackedex"), ("gear->tiered", "t2r3"), ("rr", "t3"), ("leveling->rare->armours", "shields"),
            ("leveling->rare->armours", "general"), ("leveling->rare->armours", "quivers"), ("leveling->rare->armours", "bodyarmours"),
            ("leveling->rare->armours", "bootsfocus"), ("leveling->rare->caster", "general"), ("leveling->rare->caster", "earlylevels"),
            ("leveling->normalmagic->4l", "general")] {
            assert!(has_commented(&segs, &p(path), tier), "{path} {tier}");
        }
    }
}
