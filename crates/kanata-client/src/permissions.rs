//! Can Kanata run without root here? (feature 007, research R8). Looks at the
//! system only, never starts Kanata, and never changes anything: the fix is
//! shown for the user to apply (FR-010–FR-012).

use std::ffi::CString;
use std::path::Path;

/// What was found on this system.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub uinput_writable: bool,
    pub input_readable: bool,
    /// The user's groups in the user database (`/etc/group`).
    pub db_groups: Vec<String>,
    /// The groups of this session (the running process).
    pub session_groups: Vec<String>,
    pub nixos: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// Kanata reads `/dev/input/event*`.
    InputDevices,
    /// Kanata writes `/dev/uinput`.
    VirtualKeyboard,
}

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Item::InputDevices => "input devices readable",
            Item::VirtualKeyboard => "virtual keyboard writable",
        }
    }

    fn group(self) -> &'static str {
        match self {
            Item::InputDevices => "input",
            Item::VirtualKeyboard => "uinput",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Missing,
    /// The group is set, but this session started before it was.
    NeedsLogin,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Missing => "missing",
            Status::NeedsLogin => "needs-login",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub items: Vec<(Item, Status)>,
    /// What to do, for this system; empty when everything is ok.
    pub fix: String,
    pub nixos: bool,
}

/// The verdict for a set of facts (pure).
pub fn report(f: &Facts) -> Report {
    let status = |ok: bool, item: Item| {
        let g = item.group();
        if ok {
            Status::Ok
        } else if f.db_groups.iter().any(|x| x == g) && !f.session_groups.iter().any(|x| x == g) {
            Status::NeedsLogin
        } else {
            Status::Missing
        }
    };
    let items = vec![
        (
            Item::InputDevices,
            status(f.input_readable, Item::InputDevices),
        ),
        (
            Item::VirtualKeyboard,
            status(f.uinput_writable, Item::VirtualKeyboard),
        ),
    ];
    let missing = items.iter().any(|(_, s)| *s == Status::Missing);
    let needs_login = items.iter().any(|(_, s)| *s == Status::NeedsLogin);
    let fix = if missing {
        let mut text = if f.nixos {
            "# In configuration.nix (NixOS ignores usermod and /etc/udev/rules.d):\n\
             hardware.uinput.enable = true;\n\
             users.users.<you>.extraGroups = [ \"input\" \"uinput\" ];   # keep your existing groups\n\
             # then: sudo nixos-rebuild switch, and log out and back in\n"
                .to_owned()
        } else {
            "sudo groupadd --system uinput\n\
             sudo usermod -aG input,uinput \"$USER\"\n\
             echo 'KERNEL==\"uinput\", MODE=\"0660\", GROUP=\"uinput\", OPTIONS+=\"static_node=uinput\"' \\\n  \
             | sudo tee /etc/udev/rules.d/99-input.rules\n\
             sudo modprobe uinput\n\
             sudo udevadm control --reload-rules && sudo udevadm trigger\n\
             # then log out and back in\n"
                .to_owned()
        };
        if needs_login {
            text.insert_str(
                0,
                "# Some groups are already set: a new login applies them.\n",
            );
        }
        text
    } else if needs_login {
        "Log out and back in: the groups are set, but this session started before them.\n"
            .to_owned()
    } else {
        String::new()
    };
    Report {
        items,
        fix,
        nixos: f.nixos,
    }
}

/// Look at this system.
pub fn gather() -> Facts {
    let access = |path: &Path, mode: libc::c_int| {
        CString::new(path.as_os_str().as_encoded_bytes())
            // SAFETY: a valid NUL-terminated path.
            .is_ok_and(|c| unsafe { libc::access(c.as_ptr(), mode) } == 0)
    };
    let input_readable = std::fs::read_dir("/dev/input")
        .map(|dir| {
            dir.flatten().any(|e| {
                e.file_name().to_string_lossy().starts_with("event")
                    && access(&e.path(), libc::R_OK)
            })
        })
        .unwrap_or(false);
    let user = std::env::var("USER").unwrap_or_default();
    let groups = std::fs::read_to_string("/etc/group").unwrap_or_default();
    // name:x:gid:member,member
    let entries: Vec<(&str, u32, Vec<&str>)> = groups
        .lines()
        .filter_map(|l| {
            let mut f = l.split(':');
            let name = f.next()?;
            let gid = f.nth(1)?.parse().ok()?;
            let members = f
                .next()
                .unwrap_or("")
                .split(',')
                .filter(|m| !m.is_empty())
                .collect();
            Some((name, gid, members))
        })
        .collect();
    let db_groups = entries
        .iter()
        .filter(|(_, _, m)| m.contains(&user.as_str()))
        .map(|(n, _, _)| (*n).to_owned())
        .collect();
    // SAFETY: getgroups with a correctly sized buffer.
    let session_gids: Vec<libc::gid_t> = unsafe {
        let n = libc::getgroups(0, std::ptr::null_mut());
        let mut v = vec![0; n.max(0) as usize];
        let got = libc::getgroups(n, v.as_mut_ptr());
        v.truncate(got.max(0) as usize);
        v
    };
    let session_groups = entries
        .iter()
        .filter(|(_, gid, _)| session_gids.contains(gid))
        .map(|(n, _, _)| (*n).to_owned())
        .collect();
    Facts {
        uinput_writable: access(Path::new("/dev/uinput"), libc::W_OK),
        input_readable,
        db_groups,
        session_groups,
        nixos: Path::new("/etc/NIXOS").exists(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(uinput: bool, input: bool, db: &[&str], session: &[&str], nixos: bool) -> Facts {
        Facts {
            uinput_writable: uinput,
            input_readable: input,
            db_groups: db.iter().map(|s| (*s).to_owned()).collect(),
            session_groups: session.iter().map(|s| (*s).to_owned()).collect(),
            nixos,
        }
    }

    #[test]
    fn all_ok() {
        let r = report(&facts(
            true,
            true,
            &["input", "uinput"],
            &["input", "uinput"],
            true,
        ));
        assert!(r.items.iter().all(|(_, s)| *s == Status::Ok));
        assert!(r.fix.is_empty());
    }

    #[test]
    fn missing_uinput_nixos_and_not() {
        let r = report(&facts(false, true, &["input"], &["input"], true));
        assert_eq!(r.items[1], (Item::VirtualKeyboard, Status::Missing));
        assert!(r.fix.contains("hardware.uinput.enable = true;"));
        assert!(
            !r.fix.contains("sudo usermod"),
            "NixOS gets configuration.nix, not commands"
        );
        let r = report(&facts(false, true, &["input"], &["input"], false));
        assert!(r.fix.contains("sudo usermod -aG input,uinput"));
    }

    #[test]
    fn needs_login() {
        let r = report(&facts(false, false, &["input", "uinput"], &[], true));
        assert!(r.items.iter().all(|(_, s)| *s == Status::NeedsLogin));
        assert!(r.fix.starts_with("Log out and back in"));
        let r = report(&facts(false, true, &[], &[], false));
        assert_eq!(r.items[0].1, Status::Ok);
        assert_eq!(r.items[1].1, Status::Missing);
    }

    #[test]
    fn gather_runs() {
        let f = gather();
        // Whatever this machine is, the report is consistent with it.
        let r = report(&f);
        assert_eq!(
            r.fix.is_empty(),
            r.items.iter().all(|(_, s)| *s == Status::Ok)
        );
    }
}
