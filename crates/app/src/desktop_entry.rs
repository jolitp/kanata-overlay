//! The app's desktop entry, icon and login entry (feature 007, research R6, R7,
//! R9). Everything is written in the user's home, only when it changed.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

/// The desktop file name; must match the app id set with
/// `set_desktop_file_name` (the portal looks it up by that name).
pub const APP_ID: &str = "io.github.jolitp.kanataoverlay";
const ICON: &str = include_str!("../../../assets/io.github.jolitp.kanataoverlay.svg");

/// `Exec` value: the path quoted per the desktop entry spec.
fn exec_value(exec: &Path) -> String {
    let s = exec.to_string_lossy();
    if s.chars().any(|c| " \t\n\"'\\><~|&;$*?#()`".contains(c)) {
        let escaped: String = s
            .chars()
            .flat_map(|c| match c {
                '"' | '`' | '$' | '\\' => vec!['\\', c],
                _ => vec![c],
            })
            .collect();
        format!("\"{escaped}\"")
    } else {
        s.into_owned()
    }
}

/// The application menu entry.
pub fn desktop_entry(exec: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Kanata Overlay\n\
         Comment=On-screen layers for the Kanata key remapper\n\
         Exec={}\n\
         Icon={APP_ID}\n\
         Categories=Utility;\n\
         StartupNotify=false\n",
        exec_value(exec)
    )
}

/// The login entry: the same, started after the panel (for the tray).
pub fn autostart_entry(exec: &Path) -> String {
    desktop_entry(exec) + "X-KDE-autostart-after=panel\n"
}

/// `$XDG_DATA_HOME`, else `~/.local/share`.
pub fn data_home() -> Option<PathBuf> {
    xdg_dir("XDG_DATA_HOME", ".local/share")
}

/// `$XDG_CONFIG_HOME`, else `~/.config`.
pub fn config_home() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

fn xdg_dir(var: &str, fallback: &str) -> Option<PathBuf> {
    match std::env::var_os(var).filter(|v| !v.is_empty()) {
        Some(dir) => Some(PathBuf::from(dir)),
        None => Some(PathBuf::from(std::env::var_os("HOME")?).join(fallback)),
    }
}

/// The login entry's path under `config_home`.
pub fn autostart_path(config_home: &Path) -> PathBuf {
    config_home
        .join("autostart")
        .join(format!("{APP_ID}.desktop"))
}

/// Write `text` to `path` unless it already holds exactly that; atomic.
/// Returns whether it wrote.
pub fn write_if_changed(path: &Path, text: &str) -> io::Result<bool> {
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(false);
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id()
    ));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map(|()| true)
}

/// Install the desktop entry and icon under `data_home` (FR-009).
pub fn install(data_home: &Path, exec: &Path) -> io::Result<()> {
    write_if_changed(
        &data_home
            .join("applications")
            .join(format!("{APP_ID}.desktop")),
        &desktop_entry(exec),
    )?;
    write_if_changed(
        &data_home
            .join("icons/hicolor/scalable/apps")
            .join(format!("{APP_ID}.svg")),
        ICON,
    )?;
    Ok(())
}

/// The launcher script for a development build (research R9): the Qt paths
/// of this environment and the Kanata it would use, then the executable.
pub fn launch_script(exe: &Path, env: &[(&str, String)], kanata: Option<&Path>) -> String {
    let quote = |s: &str| format!("'{}'", s.replace('\'', r"'\''"));
    let mut out = String::from(
        "#!/bin/sh\n# Written by kanata-overlay at every start (development build): the Qt\n\
         # paths of the shell it last ran in. Stale after a Nix garbage collection;\n\
         # install the flake package for a lasting entry.\n",
    );
    for (name, value) in env {
        out += &format!("export {name}={}\n", quote(value));
    }
    out += &format!("exec {}", quote(&exe.to_string_lossy()));
    if let Some(k) = kanata {
        out += &format!(" --kanata {}", quote(&k.to_string_lossy()));
    }
    out += " \"$@\"\n";
    out
}

/// Where the desktop and login entries point (research R9): the installed
/// wrapper, or a launcher script for a development build.
pub fn launch_target(data_home: &Path, kanata_arg: Option<&Path>) -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    if let Some(wrapper) = std::env::var_os("KANATA_OVERLAY_WRAPPER")
        && exe.starts_with("/nix/store")
    {
        return Ok(PathBuf::from(wrapper));
    }
    let env: Vec<(&str, String)> = [
        "QT_PLUGIN_PATH",
        "QML2_IMPORT_PATH",
        "QML_IMPORT_PATH",
        "QT_QPA_PLATFORMTHEME",
    ]
    .into_iter()
    .filter_map(|v| Some((v, std::env::var(v).ok().filter(|s| !s.is_empty())?)))
    .collect();
    let kanata = kanata_arg
        .map(Path::to_path_buf)
        .or_else(|| find_on_path("kanata"));
    let script = data_home.join("kanata-overlay").join("launch.sh");
    write_if_changed(&script, &launch_script(&exe, &env, kanata.as_deref()))?;
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    Ok(script)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
        .and_then(|p| std::fs::canonicalize(p).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries() {
        let e = desktop_entry(Path::new("/home/me/.local/share/kanata-overlay/launch.sh"));
        assert!(e.starts_with("[Desktop Entry]\nType=Application\n"));
        assert!(e.contains("\nExec=/home/me/.local/share/kanata-overlay/launch.sh\n"));
        assert!(e.contains(&format!("\nIcon={APP_ID}\n")));
        assert!(!e.contains("autostart"));
        let a = autostart_entry(Path::new("/x"));
        assert!(a.ends_with("X-KDE-autostart-after=panel\n"));
        assert!(
            desktop_entry(Path::new("/my apps/run \"it\""))
                .contains("Exec=\"/my apps/run \\\"it\\\"\"\n")
        );
    }

    #[test]
    fn launcher_script() {
        let s = launch_script(
            Path::new("/p/kanata-overlay"),
            &[
                ("QT_PLUGIN_PATH", "/q/plugins".into()),
                ("QML2_IMPORT_PATH", "/q/it's".into()),
            ],
            Some(Path::new("/k/bin/kanata")),
        );
        assert!(s.starts_with("#!/bin/sh\n"));
        assert!(s.contains("export QT_PLUGIN_PATH='/q/plugins'\n"));
        assert!(s.contains("export QML2_IMPORT_PATH='/q/it'\\''s'\n"));
        assert!(s.ends_with("exec '/p/kanata-overlay' --kanata '/k/bin/kanata' \"$@\"\n"));
        assert!(!launch_script(Path::new("/p"), &[], None).contains("--kanata"));
    }

    #[test]
    fn writes_only_when_changed() {
        let dir = std::env::temp_dir().join(format!("desktop-entry-{}", std::process::id()));
        install(&dir, Path::new("/x/kanata-overlay")).unwrap();
        let entry = dir.join("applications").join(format!("{APP_ID}.desktop"));
        let icon = dir.join(format!("icons/hicolor/scalable/apps/{APP_ID}.svg"));
        assert!(icon.is_file());
        assert!(!write_if_changed(&entry, &desktop_entry(Path::new("/x/kanata-overlay"))).unwrap());
        assert!(write_if_changed(&entry, &desktop_entry(Path::new("/y/kanata-overlay"))).unwrap());
        assert!(
            std::fs::read_to_string(&entry)
                .unwrap()
                .contains("Exec=/y/kanata-overlay")
        );
        assert_eq!(
            autostart_path(Path::new("/c")),
            PathBuf::from(format!("/c/autostart/{APP_ID}.desktop"))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
