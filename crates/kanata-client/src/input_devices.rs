//! Which input devices exist, and which ones a Kanata config would grab
//! (feature 004, research R5). Reads `/proc/bus/input/devices`, which needs no
//! permissions. Several devices can share a name, so everything here works on
//! devices (their `eventN`), not names.

use std::io;

use kbd::DeviceSelection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDevice {
    pub name: String,
    /// `eventN` handlers, e.g. `event4`.
    pub events: Vec<String>,
    /// Has keys and the letter keys A–Z.
    pub keyboard: bool,
    /// Has relative axes and a left button.
    pub mouse: bool,
}

/// The devices present now.
pub fn input_devices() -> io::Result<Vec<InputDevice>> {
    Ok(parse_input_devices(&std::fs::read_to_string(
        "/proc/bus/input/devices",
    )?))
}

/// The selection with `linux-dev` links (`/dev/input/by-id/…`) resolved to the
/// `eventN` they point at, so [`grabs`] can match them. A path that doesn't
/// resolve (device unplugged) is kept as written.
pub fn resolve_links(sel: &DeviceSelection) -> DeviceSelection {
    let mut sel = sel.clone();
    for path in &mut sel.paths {
        if let Ok(real) = std::fs::canonicalize(&*path) {
            *path = real.to_string_lossy().into_owned();
        }
    }
    sel
}

/// Parse the text of `/proc/bus/input/devices`.
pub fn parse_input_devices(text: &str) -> Vec<InputDevice> {
    text.split("\n\n")
        .filter_map(|block| {
            let mut name = None;
            let mut events = Vec::new();
            let (mut ev, mut key) = (Vec::new(), Vec::new());
            for line in block.lines() {
                if let Some(n) = line.strip_prefix("N: Name=") {
                    name = Some(n.trim().trim_matches('"').to_owned());
                } else if let Some(h) = line.strip_prefix("H: Handlers=") {
                    events = h
                        .split_whitespace()
                        .filter(|w| w.starts_with("event"))
                        .map(str::to_owned)
                        .collect();
                } else if let Some(b) = line.strip_prefix("B: EV=") {
                    ev = bitmap(b);
                } else if let Some(b) = line.strip_prefix("B: KEY=") {
                    key = bitmap(b);
                }
            }
            let has =
                |bits: &[u64], n: usize| bits.get(n / 64).is_some_and(|w| w & (1 << (n % 64)) != 0);
            // Linux key codes of Q…P, A…L, Z…M (KEY_Q = 16 … KEY_M = 50).
            let letters = (16..=25).chain(30..=38).chain(44..=50);
            Some(InputDevice {
                name: name?,
                events,
                keyboard: has(&ev, 1) && letters.clone().all(|k| has(&key, k)),
                mouse: has(&ev, 2) && has(&key, 272),
            })
        })
        .collect()
}

/// Hex words, most significant first → least significant first.
fn bitmap(text: &str) -> Vec<u64> {
    text.split_whitespace()
        .rev()
        .map(|w| u64::from_str_radix(w, 16).unwrap_or(0))
        .collect()
}

/// The devices a config would grab (data-model › Grab resolution).
pub fn grabs<'a>(sel: &DeviceSelection, present: &'a [InputDevice]) -> Vec<&'a InputDevice> {
    if !sel.paths.is_empty() {
        let wanted: Vec<&str> = sel
            .paths
            .iter()
            .map(|p| p.rsplit('/').next().unwrap_or(p))
            .collect();
        present
            .iter()
            .filter(|d| d.events.iter().any(|e| wanted.contains(&e.as_str())))
            .collect()
    } else if !sel.include.is_empty() {
        present
            .iter()
            .filter(|d| sel.include.contains(&d.name))
            .collect()
    } else {
        present
            .iter()
            .filter(|d| d.keyboard || (sel.mouse_buttons && d.mouse))
            .filter(|d| !sel.exclude.contains(&d.name))
            .collect()
    }
}

/// Devices both selections would grab.
pub fn conflicts<'a>(
    a: &DeviceSelection,
    b: &DeviceSelection,
    present: &'a [InputDevice],
) -> Vec<&'a InputDevice> {
    let b = grabs(b, present);
    grabs(a, present)
        .into_iter()
        .filter(|d| b.iter().any(|o| o.events == d.events))
        .collect()
}

/// What the overlay should do about one device's Kanata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceVerdict {
    pub id: String,
    /// The config doesn't limit itself: the names it would grab.
    pub unlimited: Option<String>,
    /// An earlier device already grabs some of the same devices: its id.
    pub blocked_by: Option<String>,
    pub warnings: Vec<String>,
}

/// The device check (FR-011). `devices` is `(id, config file name, selection)`
/// in list order; an earlier device wins an overlap.
pub fn check_devices(
    devices: &[(String, String, DeviceSelection)],
    present: &[InputDevice],
) -> Vec<DeviceVerdict> {
    let names = |list: &[&InputDevice]| {
        let mut n: Vec<&str> = list.iter().map(|d| d.name.as_str()).collect();
        n.dedup();
        n.join(", ")
    };
    devices
        .iter()
        .enumerate()
        .map(|(i, (id, config, sel))| {
            let mut verdict = DeviceVerdict {
                id: id.clone(),
                ..Default::default()
            };
            if sel.paths.is_empty() && sel.include.is_empty() {
                let all = names(&grabs(sel, present));
                verdict
                    .warnings
                    .push(format!("{config} would grab every keyboard: {all}"));
                verdict.unlimited = Some(all);
            }
            for (other_id, other_config, other) in devices[..i].iter() {
                let shared = conflicts(sel, other, present);
                if !shared.is_empty() {
                    verdict.warnings.push(format!(
                        "{config} and {other_config} both grab {}; {config} was not started",
                        names(&shared)
                    ));
                    verdict.blocked_by = Some(other_id.clone());
                    break;
                }
            }
            verdict
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/proc-input-devices.txt");

    fn present() -> Vec<InputDevice> {
        parse_input_devices(FIXTURE)
    }

    fn events(list: &[&InputDevice]) -> Vec<String> {
        let mut e: Vec<String> = list.iter().flat_map(|d| d.events.clone()).collect();
        e.sort();
        e
    }

    fn include(names: &[&str]) -> DeviceSelection {
        DeviceSelection {
            include: names.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    const DEFY: &str = "DYGMA DEFY Keyboard";
    const RECEIVER: &str = "Compx 2.4G Wireless Receiver";
    const KEYPAD: &str = "Compx 2.4G Wireless Receiver Keyboard";

    #[test]
    fn parses_the_users_devices() {
        let p = present();
        assert_eq!(p.len(), 7);
        let receivers: Vec<_> = p.iter().filter(|d| d.name == RECEIVER).collect();
        assert_eq!(events(&receivers), ["event3", "event6", "event7"]);
        let kb: Vec<_> = p
            .iter()
            .filter(|d| d.keyboard)
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(kb, ["AT Translated Set 2 keyboard", KEYPAD, DEFY]);
        let mice: Vec<_> = p
            .iter()
            .filter(|d| d.mouse)
            .flat_map(|d| d.events.clone())
            .collect();
        assert_eq!(mice, ["event3", "event15"]);
    }

    #[test]
    fn grabs_by_name_path_and_auto() {
        let p = present();
        assert_eq!(
            events(&grabs(&include(&[RECEIVER, KEYPAD]), &p)),
            ["event3", "event4", "event6", "event7"]
        );
        assert_eq!(events(&grabs(&include(&[DEFY]), &p)), ["event13"]);
        let auto = DeviceSelection::default();
        assert_eq!(events(&grabs(&auto, &p)), ["event0", "event13", "event4"]);
        let auto_mouse = DeviceSelection {
            mouse_buttons: true,
            ..Default::default()
        };
        assert_eq!(
            events(&grabs(&auto_mouse, &p)),
            ["event0", "event13", "event15", "event3", "event4"]
        );
        let excluding = DeviceSelection {
            exclude: vec!["AT Translated Set 2 keyboard".into()],
            ..Default::default()
        };
        assert_eq!(events(&grabs(&excluding, &p)), ["event13", "event4"]);
        let by_path = DeviceSelection {
            paths: vec!["/dev/input/event13".into()],
            ..Default::default()
        };
        assert_eq!(events(&grabs(&by_path, &p)), ["event13"]);
    }

    #[test]
    fn the_users_configs_do_not_overlap() {
        let p = present();
        let verdicts = check_devices(
            &[
                ("defy".into(), "defy.kbd".into(), include(&[DEFY])),
                (
                    "mouse".into(),
                    "mouse.kbd".into(),
                    include(&[RECEIVER, KEYPAD]),
                ),
            ],
            &p,
        );
        assert!(
            verdicts
                .iter()
                .all(|v| v.warnings.is_empty() && v.blocked_by.is_none())
        );
    }

    #[test]
    fn unlimited_and_shared_configs_are_reported() {
        let p = present();
        let verdicts = check_devices(
            &[
                ("defy".into(), "defy.kbd".into(), include(&[DEFY])),
                (
                    "mouse".into(),
                    "mouse.kbd".into(),
                    DeviceSelection::default(),
                ),
                ("again".into(), "again.kbd".into(), include(&[DEFY])),
            ],
            &p,
        );
        assert_eq!(
            verdicts[0],
            DeviceVerdict {
                id: "defy".into(),
                ..Default::default()
            }
        );
        assert_eq!(verdicts[1].blocked_by.as_deref(), Some("defy"));
        assert!(
            verdicts[1]
                .unlimited
                .as_deref()
                .unwrap()
                .contains("AT Translated Set 2 keyboard")
        );
        assert!(verdicts[1].warnings[0].starts_with("mouse.kbd would grab every keyboard"));
        assert_eq!(verdicts[2].blocked_by.as_deref(), Some("defy"));
        assert_eq!(
            verdicts[2].warnings,
            ["again.kbd and defy.kbd both grab DYGMA DEFY Keyboard; again.kbd was not started"]
        );
    }

    #[test]
    fn by_id_links_resolve_to_their_event() {
        let dir = std::env::temp_dir().join(format!("kanata-overlay-links-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("event4"), "").unwrap();
        let link = dir.join("usb-Compx-if01-event-kbd");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("event4", &link).unwrap();
        let sel = DeviceSelection {
            paths: vec![
                link.to_string_lossy().into_owned(),
                "/nonexistent/event9".into(),
            ],
            ..Default::default()
        };
        let resolved = resolve_links(&sel);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(resolved.paths[0].ends_with("/event4"));
        assert_eq!(resolved.paths[1], "/nonexistent/event9");
        let present = present();
        assert_eq!(events(&grabs(&resolved, &present)), ["event4"]);
    }
}
