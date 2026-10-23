//! Turns one action expression into an [`Action`] (research R4, Kanata 1.12
//! `parser/src/cfg/list_actions.rs` and `KEYMODI` in `cfg/mod.rs`).

use crate::lexer::SExpr;
use crate::{Action, KeyName, Modifier, Side};

/// Modifier prefixes, longest-match first like Kanata's `KEYMODI`.
#[rustfmt::skip]
const PREFIXES: &[(&str, Modifier, Side)] = &[
    ("S-", Modifier::Shift, Side::Left), ("‹⇧", Modifier::Shift, Side::Left),
    ("⇧›", Modifier::Shift, Side::Right), ("RS-", Modifier::Shift, Side::Right),
    ("C-", Modifier::Ctrl, Side::Left), ("‹⎈", Modifier::Ctrl, Side::Left),
    ("‹⌃", Modifier::Ctrl, Side::Left), ("⎈›", Modifier::Ctrl, Side::Right),
    ("⌃›", Modifier::Ctrl, Side::Right), ("RC-", Modifier::Ctrl, Side::Right),
    ("M-", Modifier::Super, Side::Left), ("‹◆", Modifier::Super, Side::Left),
    ("‹⌘", Modifier::Super, Side::Left), ("‹❖", Modifier::Super, Side::Left),
    ("◆›", Modifier::Super, Side::Right), ("⌘›", Modifier::Super, Side::Right),
    ("❖›", Modifier::Super, Side::Right), ("RM-", Modifier::Super, Side::Right),
    ("‹⎇", Modifier::Alt, Side::Left), ("A-", Modifier::Alt, Side::Left),
    ("‹⌥", Modifier::Alt, Side::Left), ("AG-", Modifier::Alt, Side::Right),
    ("RA-", Modifier::Alt, Side::Right), ("⎇›", Modifier::Alt, Side::Right),
    ("⌥›", Modifier::Alt, Side::Right),
    // Shorter, sideless symbols last so the sided ones match first.
    ("⎈", Modifier::Ctrl, Side::Left), ("⌥", Modifier::Alt, Side::Left),
    ("⎇", Modifier::Alt, Side::Left), ("◆", Modifier::Super, Side::Left),
    ("⌘", Modifier::Super, Side::Left), ("❖", Modifier::Super, Side::Left),
];

pub(crate) fn classify(expr: &SExpr) -> Action {
    match expr {
        SExpr::Atom(a) => atom(a),
        SExpr::List(items) => list(items, expr),
    }
}

fn atom(a: &str) -> Action {
    match a {
        "_" | "‗" | "≝" => return Action::Transparent,
        "XX" | "✗" | "∅" | "•" => return Action::NoOp,
        "use-defsrc" => return Action::UseDefsrc,
        _ => {}
    }
    if let Some(name) = a.strip_prefix('@').filter(|n| !n.is_empty()) {
        return Action::Alias(name.to_owned());
    }
    if let Some(text) = a.strip_prefix('🔣').filter(|t| !t.is_empty()) {
        return Action::Unicode(text.to_owned());
    }
    if let Some((mods, key)) = strip_mods(a) {
        return Action::Combo {
            mods,
            key: KeyName::new(key),
        };
    }
    Action::Key(KeyName::new(a))
}

/// `C-S-t` → ([Ctrl, Shift], "t"); `None` if there is no prefix or no key after it.
fn strip_mods(mut rest: &str) -> Option<(Vec<(Modifier, Side)>, &str)> {
    let mut mods = Vec::new();
    while let Some((prefix, m, side)) = PREFIXES.iter().find(|(p, ..)| rest.starts_with(p)) {
        rest = &rest[prefix.len()..];
        mods.push((*m, *side));
    }
    (!mods.is_empty() && !rest.is_empty()).then_some((mods, rest))
}

fn list(items: &[SExpr], whole: &SExpr) -> Action {
    let other = || Action::Other(whole.to_string());
    let Some(head) = items.first().and_then(SExpr::atom) else {
        return other();
    };
    let arg = |i: usize| items.get(i).and_then(SExpr::atom).map(str::to_owned);

    match head {
        "layer-switch" => arg(1).map(Action::LayerSwitch).unwrap_or_else(other),
        "layer-while-held" | "layer-toggle" => arg(1).map(Action::LayerHold).unwrap_or_else(other),
        h if h.starts_with("tap-hold") || h.starts_with("tap⬓") => {
            match (items.get(3), items.get(4)) {
                (Some(tap), Some(hold)) => Action::TapHold {
                    tap: Box::new(classify(tap)),
                    hold: Box::new(classify(hold)),
                },
                _ => other(),
            }
        }
        h if h.starts_with("one-shot") && h != "one-shot-pause-processing" => {
            match items.last().filter(|_| items.len() >= 3) {
                Some(inner) => Action::OneShot(Box::new(classify(inner))),
                None => other(),
            }
        }
        "unicode" => arg(1).map(Action::Unicode).unwrap_or_else(other),
        h if h.starts_with("caps-word") => Action::CapsWord,
        "multi" => multi(&items[1..]).unwrap_or_else(other),
        _ => other(),
    }
}

/// `(multi lctl lsft t)` → Ctrl+Shift+T, if everything but the last part is a modifier.
fn multi(parts: &[SExpr]) -> Option<Action> {
    let (last, mods) = parts.split_last()?;
    if mods.is_empty() {
        return None;
    }
    let mut all = mods
        .iter()
        .map(|p| modifier_key(p.atom()?))
        .collect::<Option<Vec<_>>>()?;
    let key = match classify(last) {
        Action::Key(k) if modifier_key(&k.written).is_none() => k,
        Action::Combo { mods, key } => {
            all.extend(mods);
            key
        }
        _ => return None,
    };
    Some(Action::Combo { mods: all, key })
}

fn modifier_key(name: &str) -> Option<(Modifier, Side)> {
    Some(match crate::canonical(name).as_str() {
        "KEY_LEFTCTRL" => (Modifier::Ctrl, Side::Left),
        "KEY_RIGHTCTRL" => (Modifier::Ctrl, Side::Right),
        "KEY_LEFTSHIFT" => (Modifier::Shift, Side::Left),
        "KEY_RIGHTSHIFT" => (Modifier::Shift, Side::Right),
        "KEY_LEFTALT" => (Modifier::Alt, Side::Left),
        "KEY_RIGHTALT" => (Modifier::Alt, Side::Right),
        "KEY_LEFTMETA" => (Modifier::Super, Side::Left),
        "KEY_RIGHTMETA" => (Modifier::Super, Side::Right),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    fn act(text: &str) -> Action {
        classify(&lexer::parse(text).unwrap()[0])
    }

    fn combo(text: &str) -> (Vec<(Modifier, Side)>, String) {
        match act(text) {
            Action::Combo { mods, key } => (mods, key.written),
            other => panic!("{text}: {other:?}"),
        }
    }

    #[test]
    fn simple_atoms() {
        for t in ["_", "‗", "≝"] {
            assert_eq!(act(t), Action::Transparent);
        }
        for t in ["XX", "✗", "∅", "•"] {
            assert_eq!(act(t), Action::NoOp);
        }
        assert_eq!(act("use-defsrc"), Action::UseDefsrc);
        assert_eq!(act("@cap"), Action::Alias("cap".into()));
        assert_eq!(act("ret"), Action::Key(KeyName::new("ret")));
        assert_eq!(act("lctl"), Action::Key(KeyName::new("lctl")));
    }

    #[test]
    fn every_modifier_prefix() {
        use Modifier::*;
        use Side::*;
        #[rustfmt::skip]
        let cases = [
            ("C-c", Ctrl, Left), ("RC-c", Ctrl, Right), ("S-c", Shift, Left),
            ("RS-c", Shift, Right), ("A-c", Alt, Left), ("RA-c", Alt, Right),
            ("AG-c", Alt, Right), ("M-c", Super, Left), ("RM-c", Super, Right),
            ("‹⌃c", Ctrl, Left), ("⌃›c", Ctrl, Right), ("‹⇧c", Shift, Left),
            ("⇧›c", Shift, Right), ("‹⌥c", Alt, Left), ("⌥›c", Alt, Right),
            ("‹◆c", Super, Left), ("◆›c", Super, Right), ("‹⌘c", Super, Left),
            ("⌘›c", Super, Right), ("‹❖c", Super, Left), ("❖›c", Super, Right),
            ("‹⎈c", Ctrl, Left), ("⎈›c", Ctrl, Right), ("‹⎇c", Alt, Left),
            ("⎇›c", Alt, Right), ("⎈c", Ctrl, Left), ("⌥c", Alt, Left),
        ];
        for (text, m, side) in cases {
            assert_eq!(combo(text), (vec![(m, side)], "c".to_owned()), "{text}");
        }
    }

    #[test]
    fn stacked_prefixes() {
        assert_eq!(
            combo("C-S-t"),
            (
                vec![(Modifier::Ctrl, Side::Left), (Modifier::Shift, Side::Left)],
                "t".to_owned()
            )
        );
    }

    #[test]
    fn multi_of_modifiers_is_a_combo() {
        assert_eq!(
            combo("(multi lctl c)"),
            (vec![(Modifier::Ctrl, Side::Left)], "c".to_owned())
        );
        assert!(matches!(act("(multi a b)"), Action::Other(_)));
        assert!(matches!(act("(multi lctl lsft)"), Action::Other(_)));
    }

    #[test]
    fn layers() {
        assert_eq!(
            act("(layer-switch game)"),
            Action::LayerSwitch("game".into())
        );
        assert_eq!(
            act("(layer-while-held nav)"),
            Action::LayerHold("nav".into())
        );
        assert_eq!(act("(layer-toggle nav)"), Action::LayerHold("nav".into()));
    }

    #[test]
    fn every_tap_hold_variant() {
        for name in [
            "tap-hold",
            "tap-hold-press",
            "tap⬓↓",
            "tap-hold-release",
            "tap⬓↑",
            "tap-hold-press-timeout",
            "tap⬓↓timeout",
            "tap-hold-release-timeout",
            "tap⬓↑timeout",
            "tap-hold-release-keys",
            "tap-hold-release-tap-keys-release",
            "tap⬓↑keys",
            "tap-hold-except-keys",
            "tap⬓⤫keys",
            "tap-hold-tap-keys",
            "tap⬓tapkeys",
            "tap-hold-keys",
        ] {
            let text = format!("({name} 200 200 esc (layer-while-held nav) (a b))");
            assert_eq!(
                act(&text),
                Action::TapHold {
                    tap: Box::new(Action::Key(KeyName::new("esc"))),
                    hold: Box::new(Action::LayerHold("nav".into())),
                },
                "{name}"
            );
        }
    }

    #[test]
    fn one_shot_unicode_caps_word() {
        for name in [
            "one-shot",
            "one-shot-press",
            "one-shot↓",
            "one-shot-release-pcancel",
        ] {
            assert_eq!(
                act(&format!("({name} 500 lsft)")),
                Action::OneShot(Box::new(Action::Key(KeyName::new("lsft")))),
                "{name}"
            );
        }
        assert_eq!(act("(unicode λ)"), Action::Unicode("λ".into()));
        assert_eq!(act("🔣λ"), Action::Unicode("λ".into()));
        assert_eq!(act("(caps-word 2000)"), Action::CapsWord);
        assert_eq!(act("(caps-word-custom 2000 (a b) (c))"), Action::CapsWord);
    }

    #[test]
    fn everything_else_is_raw_text() {
        assert_eq!(act("(macro a b c)"), Action::Other("(macro a b c)".into()));
        assert!(matches!(act("(tap-dance 200 (a b))"), Action::Other(_)));
        assert!(matches!(act("(tap-hold 200 200 esc)"), Action::Other(_)));
    }
}
