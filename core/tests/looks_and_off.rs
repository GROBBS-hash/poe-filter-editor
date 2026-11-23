//! Saved looks ("My chase look" used by many edits) and edits switched off without deleting them.

use filter_core::evaluate::status;
use filter_core::filter::{parse_filter, render_filter};
use filter_core::item::parse_item;
use filter_core::profiles::{Game, Store};
use filter_core::rules::*;

fn look(text: [u8; 4], beam: &str) -> CustomStyle {
    CustomStyle { font_size: 45, text, border: Some(text), background: [20, 20, 20, 240],
        beam: Some(Beam { colour: beam.into(), temp: false }), minimap: None, sound: Sound::Default, label_hidden: false }
}

#[test]
fn older_saved_files_still_load() {
    // A settings file from before saved looks and the on/off switch.
    let old = r#"{"profiles":{"ls":{"id":"ls","name":"League Start","game":"Poe1","base_file":"a.filter","output_file":"League Start.filter",
        "parent":null,"rules":[
            {"kind":"Override","label":"Hide: Gold","show":false,"conditions":["Class == \"Gold\""],"actions":["DisableDropSound True"]},
            {"kind":"MoveBase","type_path":["currency"],"base":"Chaos Orb","to_tier":"t2"},
            {"kind":"Tier","type_path":["gold"],"tier":"stack2","show":null,"actions":null}
        ]}},"active":"ls","themes":[]}"#;
    let store: Store = serde_json::from_str(old).expect("old file loads");
    let p = &store.profiles["ls"];
    assert_eq!(p.rules.len(), 3);
    assert!(p.rules.iter().all(|r| !r.is_off() && r.look().is_none()));
    assert!(store.looks.is_empty());
    // Saving again doesn't add the new fields while they're unused, so older editors can still read it.
    let json = serde_json::to_string(&p.rules).unwrap();
    assert!(!json.contains("\"off\"") && !json.contains("\"look\""), "{json}");
}

#[test]
fn switched_off_edits_are_kept_but_not_built() {
    let gold = parse_item("Item Class: Gold\nRarity: Currency\nGold\n").unwrap();
    let mut hide = quick_rule(&gold, Action::Hide, None, &MatchOpts::default());
    let base = "Show\n    BaseType == \"Gold\"\n    SetFontSize 30\n";
    let built = |rules: &[Rule]| {
        let mut segs = parse_filter(base);
        apply(&mut segs, rules, "\n");
        status(&parse_filter(&render_filter(&segs)), &gold, Some(70), false).verdict.visibility
    };
    assert_eq!(built(std::slice::from_ref(&hide)), "Hide");
    hide.set_off(true);
    assert!(hide.is_off());
    assert_eq!(built(std::slice::from_ref(&hide)), "Show", "an edit that's off leaves NeverSink's rule in charge");
    // It survives a save and load.
    let back: Rule = serde_json::from_str(&serde_json::to_string(&hide).unwrap()).unwrap();
    assert!(back.is_off());

    // An off edit doesn't make an older one "never apply".
    let style = look([255, 0, 0, 255], "Red");
    let older = custom_rule(&gold, &style, &MatchOpts::default());
    assert_eq!(shadowed_by(&[older.clone(), hide.clone()]), vec![None, None]);
    hide.set_off(false);
    assert_eq!(shadowed_by(&[older, hide]), vec![Some(1), None]);
}

#[test]
fn a_saved_look_updates_every_edit_that_uses_it() {
    let mut store = Store::default();
    let id = store.create("League Start", Game::Poe1, "a.filter");
    let id2 = store.create("Strict", Game::Poe1, "a.filter");
    let chase = look([255, 0, 0, 255], "Red");
    assert!(store.save_look(SavedLook { name: "My chase look".into(), style: chase.clone() }, None).unwrap().is_empty());

    let divine = parse_item("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n").unwrap();
    let mirror = parse_item("Item Class: Stackable Currency\nRarity: Currency\nMirror of Kalandra\n").unwrap();
    let gold = parse_item("Item Class: Gold\nRarity: Currency\nGold\n").unwrap();
    let mut r1 = custom_rule(&divine, &chase, &MatchOpts::default());
    let mut r2 = custom_rule(&mirror, &look([1, 1, 1, 255], "White"), &MatchOpts::default());
    let mut hidden = quick_rule(&gold, Action::Hide, None, &MatchOpts::default());
    assert!(r1.use_look("My chase look", &chase));
    assert!(r2.use_look("My chase look", &chase));
    assert!(!hidden.use_look("My chase look", &chase), "a hidden edit has no look to take");
    let mut tier = Rule::Tier { type_path: vec!["currency".into()], tier: "t1".into(), show: None, actions: None,
        min_stack: None, min_item_tier: None, conds: None, off: false, look: None };
    assert!(tier.use_look("My chase look", &chase));
    store.profiles.get_mut(&id).unwrap().rules = vec![r1, hidden];
    store.profiles.get_mut(&id2).unwrap().rules = vec![r2, tier];
    assert_eq!(store.look_uses("My chase look"), 3);

    // Change the look: every edit using it, in both filters, gets the new colour.
    let gold_look = look([255, 200, 0, 255], "Yellow");
    let mut changed = store.save_look(SavedLook { name: "My chase look".into(), style: gold_look.clone() }, Some("My chase look")).unwrap();
    changed.sort();
    assert_eq!(changed, vec![id.clone(), id2.clone()]);
    let want = style_actions(&gold_look);
    let actions = |r: &Rule| match r { Rule::Override { actions, .. } => actions.clone(), Rule::Tier { actions, .. } => actions.clone().unwrap(), _ => vec![] };
    assert_eq!(actions(&store.profiles[&id].rules[0]), want);
    assert_eq!(actions(&store.profiles[&id2].rules[0]), want);
    assert_eq!(actions(&store.profiles[&id2].rules[1]), want);
    assert_eq!(actions(&store.profiles[&id].rules[1]), vec!["DisableDropSound True".to_string()], "the hide is untouched");

    // Rename: the edits follow the new name.
    store.save_look(SavedLook { name: "Top tier".into(), style: gold_look.clone() }, Some("My chase look")).unwrap();
    assert_eq!(store.looks.len(), 1);
    assert_eq!(store.look_uses("Top tier"), 3);
    assert_eq!(store.look_uses("My chase look"), 0);

    // Names must be unique (any capitals) and not empty.
    store.save_look(SavedLook { name: "Quiet".into(), style: gold_look.clone() }, None).unwrap();
    assert!(store.save_look(SavedLook { name: "top TIER".into(), style: gold_look.clone() }, None).is_err());
    assert!(store.save_look(SavedLook { name: "Quiet".into(), style: gold_look.clone() }, Some("Top tier")).is_err());
    assert!(store.save_look(SavedLook { name: "  ".into(), style: gold_look.clone() }, None).is_err());

    // Delete: edits keep their colours but stop following it.
    store.delete_look("Top tier");
    assert_eq!(store.look_uses("Top tier"), 0);
    assert!(store.profiles[&id].rules[0].look().is_none());
    assert_eq!(actions(&store.profiles[&id].rules[0]), want);

    // A store with looks round-trips through the settings file.
    let back: Store = serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
    assert_eq!(back.looks, store.looks);
}
