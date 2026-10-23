//! The top-level forms the overlay understands (research R3).

use std::collections::HashMap;

use crate::action::classify;
use crate::lexer::SExpr;
use crate::{Action, Config, ConfigError, ConfigWarning, KeyName, Layer};

/// Forms that change what keys do but that we don't evaluate; reported once.
const SKIPPED: &[&str] = &[
    "include",
    "deftemplate",
    "defaliasenvcond",
    "platform",
    "environment",
];

pub(crate) fn read(forms: &[SExpr]) -> Result<Config, ConfigError> {
    let source: Vec<KeyName> = forms
        .iter()
        .find(|f| head(f) == "defsrc")
        .ok_or(ConfigError::NoDefsrc)?
        .list()
        .unwrap_or_default()[1..]
        .iter()
        .filter_map(SExpr::atom)
        .map(KeyName::new)
        .collect();

    let mut layers = Vec::new();
    let mut aliases = HashMap::new();
    let mut warnings = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut delegate_to_first_layer = false;
    let mut devices = crate::DeviceSelection {
        mouse_buttons: source.iter().any(|k| k.canonical.starts_with("BTN_")),
        ..Default::default()
    };

    for form in forms {
        let Some(list) = items(form) else { continue };
        match head(form) {
            "deflayer" => {
                let name = layer_name(list.get(1));
                let mut actions: Vec<Action> = list[2..].iter().map(classify).collect();
                if actions.len() != source.len() {
                    warnings.push(ConfigWarning::LayerLength {
                        layer: name.clone(),
                        expected: source.len(),
                        found: actions.len(),
                    });
                    actions.resize(source.len(), Action::Transparent);
                }
                layers.push(Layer { name, actions });
            }
            "deflayermap" => layers.push(layer_map(list, &source)),
            "defalias" => {
                for pair in list[1..].chunks(2) {
                    if let [name, action] = pair
                        && let Some(name) = name.atom()
                        && aliases.insert(name.to_owned(), classify(action)).is_some()
                    {
                        warnings.push(ConfigWarning::DuplicateAlias {
                            name: name.to_owned(),
                        });
                    }
                }
            }
            "defcfg" => {
                for pair in list[1..].chunks(2) {
                    let [key, value] = pair else { continue };
                    match key.atom() {
                        Some("delegate-to-first-layer") => {
                            delegate_to_first_layer = matches!(value.atom(), Some("yes" | "true"));
                        }
                        Some("linux-dev") => devices.paths = device_paths(value),
                        Some("linux-dev-names-include") => devices.include = names(value),
                        Some("linux-dev-names-exclude") => devices.exclude = names(value),
                        _ => {}
                    }
                }
            }
            h if SKIPPED.contains(&h) && !skipped.iter().any(|s| s == h) => {
                skipped.push(h.to_owned());
            }
            _ => {}
        }
    }

    if layers.is_empty() {
        return Err(ConfigError::NoLayers);
    }
    if !skipped.is_empty() {
        warnings.insert(0, ConfigWarning::Skipped { forms: skipped });
    }
    Ok(Config {
        source,
        layers,
        aliases,
        delegate_to_first_layer,
        devices,
        warnings,
    })
}

/// `linux-dev a:b` (with `\:` escaping a colon), or a list, taken literally.
fn device_paths(value: &SExpr) -> Vec<String> {
    match value {
        SExpr::List(items) => items
            .iter()
            .filter_map(SExpr::atom)
            .map(str::to_owned)
            .collect(),
        SExpr::Atom(a) => {
            let mut out = vec![String::new()];
            let mut chars = a.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(next) = chars.next() {
                            out.last_mut().unwrap().push(next);
                        }
                    }
                    ':' => out.push(String::new()),
                    c => out.last_mut().unwrap().push(c),
                }
            }
            out.into_iter().filter(|p| !p.is_empty()).collect()
        }
    }
}

/// A list of names (a single atom is a one-name list).
fn names(value: &SExpr) -> Vec<String> {
    match value {
        SExpr::List(items) => items
            .iter()
            .filter_map(SExpr::atom)
            .map(str::to_owned)
            .collect(),
        SExpr::Atom(a) => vec![a.clone()],
    }
}

/// A non-empty list form.
fn items(f: &SExpr) -> Option<&[SExpr]> {
    f.list().filter(|l| !l.is_empty())
}

/// The first atom of a list form, e.g. `defsrc`.
fn head(f: &SExpr) -> &str {
    items(f).and_then(|l| l[0].atom()).unwrap_or("")
}

/// `deflayer name …` or `deflayer (name options…) …`.
fn layer_name(expr: Option<&SExpr>) -> String {
    match expr {
        Some(SExpr::Atom(a)) => a.clone(),
        Some(SExpr::List(l)) => l.first().and_then(SExpr::atom).unwrap_or("").to_owned(),
        None => String::new(),
    }
}

/// `deflayermap (name) key action …`, with the wildcards `_` (unmapped defsrc
/// keys) and `___` (all unmapped keys); `__` only covers keys outside defsrc.
fn layer_map(list: &[SExpr], source: &[KeyName]) -> Layer {
    let name = layer_name(list.get(1));
    let mut mapped: Vec<Option<Action>> = vec![None; source.len()];
    let (mut unmapped_src, mut unmapped_all) = (None, None);

    for pair in list[2..].chunks(2) {
        let [key, action] = pair else { continue };
        match key.atom() {
            Some("_") => unmapped_src = Some(classify(action)),
            Some("___") => unmapped_all = Some(classify(action)),
            Some("__") | None => {}
            Some(k) => {
                let k = crate::canonical(k);
                if let Some(i) = source.iter().position(|s| s.canonical == k) {
                    mapped[i] = Some(classify(action));
                }
            }
        }
    }

    let fill = unmapped_src.or(unmapped_all).unwrap_or(Action::Transparent);
    Layer {
        name,
        actions: mapped
            .into_iter()
            .map(|a| a.unwrap_or_else(|| fill.clone()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{Action, ConfigError, ConfigWarning, KeyName, parse};

    const SRC: &str = "(defsrc a b c)";

    fn key(k: &str) -> Action {
        Action::Key(KeyName::new(k))
    }

    #[test]
    fn reads_the_example_config() {
        let cfg = parse(include_str!("../../../examples/kanata/minimal.kbd")).unwrap();
        assert_eq!(cfg.source.len(), 38);
        assert_eq!(cfg.layers.len(), 2);
        assert_eq!(cfg.layers[1].name, "nav");
        assert!(cfg.aliases.contains_key("cap"));
        assert!(cfg.warnings.is_empty(), "{:?}", cfg.warnings);
    }

    #[test]
    fn short_and_long_layers_are_fixed_up() {
        let cfg = parse(&format!("{SRC} (deflayer s x) (deflayer l x y z w)")).unwrap();
        assert_eq!(
            cfg.layers[0].actions,
            vec![key("x"), Action::Transparent, Action::Transparent]
        );
        assert_eq!(cfg.layers[1].actions.len(), 3);
        assert_eq!(cfg.warnings.len(), 2);
    }

    #[test]
    fn several_alias_blocks_and_duplicates() {
        let cfg = parse(&format!(
            "{SRC} (deflayer l a b c) (defalias x a y b) (defalias x c)"
        ))
        .unwrap();
        assert_eq!(cfg.aliases["x"], key("c"));
        assert_eq!(cfg.aliases["y"], key("b"));
        assert_eq!(
            cfg.warnings,
            vec![ConfigWarning::DuplicateAlias { name: "x".into() }]
        );
    }

    #[test]
    fn deflayermap_pairs_and_wildcards() {
        let cfg = parse(&format!("{SRC} (deflayermap (m) c x a y)")).unwrap();
        assert_eq!(
            cfg.layers[0].actions,
            vec![key("y"), Action::Transparent, key("x")]
        );

        let cfg = parse(&format!("{SRC} (deflayermap (m) a y _ XX)")).unwrap();
        assert_eq!(
            cfg.layers[0].actions,
            vec![key("y"), Action::NoOp, Action::NoOp]
        );

        let cfg = parse(&format!("{SRC} (deflayermap (m) ___ z __ XX)")).unwrap();
        assert_eq!(cfg.layers[0].actions, vec![key("z"), key("z"), key("z")]);

        // A key outside defsrc, and a different spelling of one inside it.
        let cfg = parse("(defsrc ret) (deflayermap (m) f13 x enter y)").unwrap();
        assert_eq!(cfg.layers[0].actions, vec![key("y")]);
    }

    #[test]
    fn skipped_forms_are_reported_once() {
        let cfg = parse(&format!(
            "(include a.kbd) {SRC} (include b.kbd) (deftemplate t () a) (deflayer l a b c)"
        ))
        .unwrap();
        assert_eq!(
            cfg.warnings,
            vec![ConfigWarning::Skipped {
                forms: vec!["include".into(), "deftemplate".into()]
            }]
        );
    }

    #[test]
    fn reads_device_selection() {
        let devices = |cfg: &str| {
            parse(&format!("{cfg} {SRC} (deflayer l a b c)"))
                .unwrap()
                .devices
        };
        assert_eq!(
            devices("(defcfg linux-dev /dev/input/a:/dev/input/b)").paths,
            ["/dev/input/a", "/dev/input/b"]
        );
        assert_eq!(
            devices(r"(defcfg linux-dev /dev/input/by-id/x\:y)").paths,
            ["/dev/input/by-id/x:y"]
        );
        assert_eq!(
            devices(r#"(defcfg linux-dev (/dev/input/p:q "/dev/input/with space"))"#).paths,
            ["/dev/input/p:q", "/dev/input/with space"]
        );
        let d = devices(
            r#"(defcfg linux-dev-names-include ("DYGMA DEFY Keyboard" "X") linux-dev-names-exclude ("Y"))"#,
        );
        assert_eq!(d.include, ["DYGMA DEFY Keyboard", "X"]);
        assert_eq!(d.exclude, ["Y"]);
        assert!(!d.mouse_buttons);
        assert_eq!(devices(""), crate::DeviceSelection::default());

        let mouse = parse("(defsrc ret mbck mfwd) (deflayer l a b c)")
            .unwrap()
            .devices;
        assert!(mouse.mouse_buttons);
    }

    #[test]
    fn reads_delegate_to_first_layer() {
        let cfg = parse(&format!(
            "(defcfg process-unmapped-keys yes delegate-to-first-layer yes) {SRC} (deflayer l a b c)"
        ))
        .unwrap();
        assert!(cfg.delegate_to_first_layer);
    }

    #[test]
    fn missing_parts_are_errors() {
        assert_eq!(parse("(deflayer l a)"), Err(ConfigError::NoDefsrc));
        assert_eq!(parse(SRC), Err(ConfigError::NoLayers));
        assert!(matches!(parse("(defsrc a"), Err(ConfigError::Syntax(_))));
    }
}
