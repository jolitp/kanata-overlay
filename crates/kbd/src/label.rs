//! What each key shows on a layer (contracts/labels.md, data-model.md › Label
//! resolution).

use std::collections::HashMap;

use crate::{Action, Config, ConfigWarning, KeyLabel, KeyName, LabelStyle, Modifier, Side};

/// Aliases are followed this deep before we call it a loop.
const MAX_ALIAS_DEPTH: usize = 8;
/// Raw text and layer names are cut to this many characters (plus `…`).
const MAX_TEXT: usize = 8;

/// Labels for every `defsrc` key on `active_layer`, keyed by canonical key
/// name. An empty or unknown layer name gives the first layer (FR-010).
pub fn labels(
    config: &Config,
    active_layer: &str,
) -> (HashMap<String, KeyLabel>, Vec<ConfigWarning>) {
    let mut warnings = Vec::new();
    let index = match config.layers.iter().position(|l| l.name == active_layer) {
        Some(i) => i,
        None => {
            if !active_layer.is_empty() {
                warnings.push(ConfigWarning::UnknownLayer {
                    name: active_layer.to_owned(),
                });
            }
            0
        }
    };
    let layer = &config.layers[index];
    let first = &config.layers[0];
    let mut ctx = Ctx {
        config,
        warnings: &mut warnings,
    };

    let mut out = HashMap::new();
    for (i, key) in config.source.iter().enumerate() {
        let (mut action, mut alias) = ctx.resolve(&layer.actions[i]);
        let mut style = LabelStyle::Normal;

        if action == Action::Transparent {
            // Approximate Kanata's layer stack as active → first → defsrc (research R6).
            style = LabelStyle::Inherited;
            (action, alias) = if index == 0 {
                (Action::UseDefsrc, None)
            } else {
                ctx.resolve(&first.actions[i])
            };
            if action == Action::Transparent {
                action = Action::UseDefsrc;
            }
        }
        if action == Action::UseDefsrc {
            style = LabelStyle::Inherited;
            action = Action::Key(key.clone());
        }

        let label = match &action {
            Action::NoOp => KeyLabel {
                text: String::new(),
                style: LabelStyle::Disabled,
                hold: false,
                alias,
            },
            Action::TapHold { tap, .. } => KeyLabel {
                text: ctx.text(tap, 0),
                style,
                hold: true,
                alias,
            },
            other => KeyLabel {
                text: ctx.text(other, 0),
                style,
                hold: false,
                alias,
            },
        };
        out.insert(key.canonical.clone(), label);
    }

    warnings.dedup();
    (out, warnings)
}

struct Ctx<'a> {
    config: &'a Config,
    warnings: &'a mut Vec<ConfigWarning>,
}

impl Ctx<'_> {
    /// Follow aliases. Returns the action and the first alias name used.
    fn resolve(&mut self, action: &Action) -> (Action, Option<String>) {
        let mut current = action.clone();
        let mut first_alias = None;
        let mut seen = Vec::new();
        while let Action::Alias(name) = &current {
            first_alias.get_or_insert_with(|| name.clone());
            if seen.contains(name) || seen.len() >= MAX_ALIAS_DEPTH {
                self.warn(ConfigWarning::AliasLoop { name: name.clone() });
                return (Action::Other(format!("@{name}")), None);
            }
            seen.push(name.clone());
            match self.config.aliases.get(name) {
                Some(next) => current = next.clone(),
                None => {
                    self.warn(ConfigWarning::UnknownAlias { name: name.clone() });
                    return (Action::Other(format!("@{name}")), None);
                }
            }
        }
        (current, first_alias)
    }

    fn warn(&mut self, w: ConfigWarning) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    /// Short label text for an action (contracts/labels.md).
    fn text(&mut self, action: &Action, depth: usize) -> String {
        if depth > MAX_ALIAS_DEPTH {
            return "…".to_owned();
        }
        match action {
            Action::Key(k) => key_text(k),
            Action::Combo { mods, key } => {
                let mut mods = mods.clone();
                mods.sort();
                mods.dedup();
                let mut s: String = mods
                    .iter()
                    .map(|&(m, side)| modifier_text(m, side))
                    .collect();
                s.push_str(&key_text(key));
                s
            }
            Action::LayerHold(name) => format!("↥{}", shorten(name)),
            Action::LayerSwitch(name) => format!("⇒{}", shorten(name)),
            Action::TapHold { tap, .. } => self.text(tap, depth + 1),
            Action::OneShot(inner) => format!("¹{}", self.text(inner, depth + 1)),
            Action::Unicode(s) => s.clone(),
            Action::CapsWord => "⇪W".to_owned(),
            Action::Alias(_) => {
                let (resolved, _) = self.resolve(action);
                self.text(&resolved, depth + 1)
            }
            Action::Other(raw) => shorten(raw),
            Action::Transparent | Action::NoOp | Action::UseDefsrc => String::new(),
        }
    }
}

fn modifier_text(m: Modifier, side: Side) -> String {
    let symbol = match m {
        Modifier::Ctrl => "⌃",
        Modifier::Alt => "⌥",
        Modifier::Shift => "⇧",
        Modifier::Super => "◆",
    };
    match side {
        Side::Left => symbol.to_owned(),
        Side::Right => format!("{symbol}›"),
    }
}

/// A key's glyph or short name.
fn key_text(k: &KeyName) -> String {
    let code = k.canonical.as_str();
    if let Some(rest) = code.strip_prefix("KEY_") {
        // Letters and digits: KEY_A … KEY_Z, KEY_0 … KEY_9.
        if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_alphanumeric()) {
            return rest.to_owned();
        }
        // KEY_F1 … KEY_F24.
        if let Some(n) = rest.strip_prefix('F')
            && n.parse::<u8>().is_ok_and(|n| (1..=24).contains(&n))
        {
            return rest.to_owned();
        }
    }
    #[rustfmt::skip]
    let glyph = match code {
        "KEY_ENTER" => "⏎", "KEY_BACKSPACE" => "⌫", "KEY_DELETE" => "⌦",
        "KEY_TAB" => "⇥", "KEY_SPACE" => "␣", "KEY_ESC" => "Esc",
        "KEY_CAPSLOCK" => "⇪", "KEY_LEFT" => "←", "KEY_RIGHT" => "→",
        "KEY_UP" => "↑", "KEY_DOWN" => "↓", "KEY_PAGEUP" => "⇞",
        "KEY_PAGEDOWN" => "⇟", "KEY_HOME" => "⇱", "KEY_END" => "⇲",
        "KEY_INSERT" => "Ins",
        "KEY_LEFTSHIFT" => "⇧", "KEY_RIGHTSHIFT" => "⇧›",
        "KEY_LEFTCTRL" => "⌃", "KEY_RIGHTCTRL" => "⌃›",
        "KEY_LEFTALT" => "⌥", "KEY_RIGHTALT" => "⌥›",
        "KEY_LEFTMETA" => "◆", "KEY_RIGHTMETA" => "◆›",
        "KEY_VOLUMEUP" => "🔊+", "KEY_VOLUMEDOWN" => "🔊−", "KEY_MUTE" => "🔇",
        "KEY_PLAYPAUSE" => "⏯",
        "KEY_SEMICOLON" => ";", "KEY_COMMA" => ",", "KEY_DOT" => ".",
        "KEY_SLASH" => "/", "KEY_MINUS" => "-", "KEY_EQUAL" => "=",
        "KEY_LEFTBRACE" => "[", "KEY_RIGHTBRACE" => "]", "KEY_APOSTROPHE" => "'",
        "KEY_GRAVE" => "`", "KEY_BACKSLASH" => "\\",
        _ => return shorten(&k.written),
    };
    glyph.to_owned()
}

fn shorten(s: &str) -> String {
    if s.chars().count() <= MAX_TEXT {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(MAX_TEXT).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    const MINIMAL: &str = include_str!("../../../examples/kanata/minimal.kbd");

    fn label(cfg: &str, layer: &str, key: &str) -> KeyLabel {
        let cfg = parse(cfg).unwrap();
        let (labels, _) = labels(&cfg, layer);
        labels[&crate::canonical(key)].clone()
    }

    fn text(cfg: &str, layer: &str, key: &str) -> String {
        label(cfg, layer, key).text
    }

    #[test]
    fn minimal_base_layer() {
        for (key, want) in [
            ("a", "A"),
            ("bspc", "⌫"),
            ("ret", "⏎"),
            ("spc", "␣"),
            ("esc", "Esc"),
            (";", ";"),
        ] {
            assert_eq!(text(MINIMAL, "base", key), want, "{key}");
        }
        let caps = label(MINIMAL, "base", "caps");
        assert_eq!(caps.text, "Esc");
        assert!(caps.hold);
        assert_eq!(caps.alias.as_deref(), Some("cap"));
        assert_eq!(caps.style, LabelStyle::Normal);
    }

    #[test]
    fn minimal_nav_layer() {
        for (key, want) in [("h", "←"), ("j", "↓"), ("k", "↑"), ("l", "→")] {
            let l = label(MINIMAL, "nav", key);
            assert_eq!(
                (l.text.as_str(), l.style),
                (want, LabelStyle::Normal),
                "{key}"
            );
        }
        let q = label(MINIMAL, "nav", "q");
        assert_eq!((q.text.as_str(), q.style), ("Q", LabelStyle::Inherited));
        // Inherited from the base layer's alias.
        assert_eq!(label(MINIMAL, "nav", "caps").text, "Esc");
    }

    #[test]
    fn unknown_or_missing_layer_shows_the_first() {
        let cfg = parse(MINIMAL).unwrap();
        let (l, w) = labels(&cfg, "gaming");
        assert_eq!(l[&crate::canonical("a")].text, "A");
        assert_eq!(
            w,
            vec![ConfigWarning::UnknownLayer {
                name: "gaming".into()
            }]
        );
        let (_, w) = labels(&cfg, "");
        assert!(w.is_empty());
    }

    #[test]
    fn transparent_noop_use_defsrc() {
        let cfg = "(defsrc a b c) (deflayer base _ XX use-defsrc) (deflayer up _ _ x)";
        let a = label(cfg, "base", "a");
        assert_eq!((a.text.as_str(), a.style), ("A", LabelStyle::Inherited));
        let b = label(cfg, "base", "b");
        assert_eq!((b.text.as_str(), b.style), ("", LabelStyle::Disabled));
        let c = label(cfg, "base", "c");
        assert_eq!((c.text.as_str(), c.style), ("C", LabelStyle::Inherited));
        // Falls through `up` → `base` (transparent too) → defsrc.
        let a = label(cfg, "up", "a");
        assert_eq!((a.text.as_str(), a.style), ("A", LabelStyle::Inherited));
        assert_eq!(label(cfg, "up", "b").style, LabelStyle::Disabled);
    }

    #[test]
    fn aliases_chains_loops_and_unknowns() {
        let cfg = "(defsrc a b c d) (deflayer l @x @p @u d) (defalias x @y y ret p @q q @p)";
        let a = label(cfg, "l", "a");
        assert_eq!((a.text.as_str(), a.alias.as_deref()), ("⏎", Some("x")));
        assert_eq!(text(cfg, "l", "b"), "@p");
        assert_eq!(text(cfg, "l", "c"), "@u");
        let (_, w) = labels(&parse(cfg).unwrap(), "l");
        assert!(w.contains(&ConfigWarning::AliasLoop { name: "p".into() }));
        assert!(w.contains(&ConfigWarning::UnknownAlias { name: "u".into() }));
    }

    #[test]
    fn spellings_collapse_to_one_key() {
        assert_eq!(text("(defsrc enter) (deflayer l ret)", "l", "ret"), "⏎");
    }

    #[test]
    fn unsupported_forms_dont_empty_the_board() {
        let cfg = "(include other.kbd) (deftemplate t (x) $x) (defsrc a b) (deflayer l a XX)";
        let parsed = parse(cfg).unwrap();
        assert_eq!(
            parsed.warnings,
            vec![ConfigWarning::Skipped {
                forms: vec!["include".into(), "deftemplate".into()]
            }]
        );
        assert_eq!(text(cfg, "l", "a"), "A");
    }

    #[test]
    fn every_label_row() {
        let cfg = "(defsrc a b c d e f g h i j k)
            (deflayer l C-c C-S-t RA-e (layer-while-held nav) (layer-switch game)
                (one-shot 500 lsft) (unicode λ) (caps-word 2000) (macro h e l l o)
                (layer-switch averyverylongname) (multi lctl lalt del))
            (deflayer nav _ _ _ _ _ _ _ _ _ _ _) (deflayer game _ _ _ _ _ _ _ _ _ _ _)";
        let want = [
            ("a", "⌃C"),
            ("b", "⌃⇧T"),
            ("c", "⌥›E"),
            ("d", "↥nav"),
            ("e", "⇒game"),
            ("f", "¹⇧"),
            ("g", "λ"),
            ("h", "⇪W"),
            ("i", "(macro h…"),
            ("j", "⇒averyver…"),
            ("k", "⌃⌥⌦"),
        ];
        for (key, want) in want {
            assert_eq!(text(cfg, "l", key), want, "{key}");
        }
    }

    #[test]
    fn all_actions_example() {
        let cfg = include_str!("../../../examples/kanata/all-actions.kbd");
        let want = [
            ("esc", "⌃C"),
            ("q", "↥nav"),
            ("w", "⇒game"),
            ("e", "Esc"),
            ("r", "⌃V"),
            ("t", "¹⇧"),
            ("y", "λ"),
            ("u", "⇪W"),
            ("i", "(macro h…"),
            ("o", ""),
        ];
        for (key, want) in want {
            assert_eq!(text(cfg, "base", key), want, "{key}");
        }
        assert!(label(cfg, "base", "e").hold);
        assert_eq!(label(cfg, "base", "r").alias.as_deref(), Some("cp"));
        assert_eq!(label(cfg, "base", "o").style, LabelStyle::Disabled);
        assert_eq!(text(cfg, "nav", "h"), "←");
        assert_eq!(label(cfg, "nav", "a").style, LabelStyle::Inherited);
        assert_eq!(text(cfg, "game", "spc"), "␣");
    }
}
