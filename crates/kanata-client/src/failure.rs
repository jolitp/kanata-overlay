//! Why a managed Kanata didn't start or stopped (research R4), from its exit
//! and output. Kanata's output is matched here and nowhere else (constitution II).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    NotInstalled {
        program: String,
    },
    InvalidConfig {
        summary: String,
    },
    Permissions,
    PortInUse {
        port: u16,
    },
    NoAnswer,
    /// None of the config's devices is connected (Kanata 1.12 exits unless
    /// `linux-continue-if-no-devs-found yes`). Retried, never given up on.
    NoDevice,
    Exited {
        code: Option<i32>,
    },
    /// Too many failures in a row; the last one is kept.
    GaveUp(Box<Failure>),
}

impl Failure {
    /// Would a restart plausibly help?
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Failure::PortInUse { .. }
                | Failure::NoAnswer
                | Failure::NoDevice
                | Failure::Exited { .. }
        )
    }

    /// What the user sees (data-model › Failure).
    pub fn notice(&self) -> String {
        match self {
            Failure::NotInstalled { program } => {
                format!("Kanata is not installed or not on PATH ({program})")
            }
            Failure::InvalidConfig { summary } => format!("Kanata config error: {summary}"),
            Failure::Permissions => "Kanata may not read the keyboard or create its virtual keyboard: see Settings › Permissions".to_owned(),
            Failure::PortInUse { port } => format!("port {port} is in use"),
            Failure::NoAnswer => "Kanata did not answer within 10 s".to_owned(),
            Failure::NoDevice => "waiting for its device (not connected)".to_owned(),
            Failure::Exited { code: Some(c) } => format!("Kanata stopped (code {c})"),
            Failure::Exited { code: None } => "Kanata stopped (killed by a signal)".to_owned(),
            Failure::GaveUp(_) => "Kanata keeps stopping; see the log".to_owned(),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.notice())
    }
}

/// The result of `kanata --check`: `None` if the config is valid.
pub(crate) fn classify_check(success: bool, output: &[String]) -> Option<Failure> {
    if success {
        return None;
    }
    let after = |marker: &str| {
        output.iter().find_map(|l| {
            l.find(marker)
                .map(|i| {
                    l[i + marker.len()..]
                        .trim_start_matches([' ', '×'])
                        .trim()
                        .to_owned()
                })
                .filter(|s| !s.is_empty())
        })
    };
    let summary = after("help:")
        .or_else(|| after("[ERROR]"))
        .unwrap_or_else(|| "kanata --check failed".to_owned());
    Some(Failure::InvalidConfig { summary })
}

/// Why Kanata exited, from its exit code and recent output.
pub(crate) fn classify_exit(code: Option<i32>, output: &[String], port: u16) -> Failure {
    if output.iter().any(|l| l.contains("Permission denied")) {
        Failure::Permissions
    } else if output
        .iter()
        .any(|l| l.contains("No keyboard devices were found"))
    {
        Failure::NoDevice
    } else if output
        .iter()
        .any(|l| l.contains("Address already in use") || l.contains("AddrInUse"))
    {
        Failure::PortInUse { port }
    } else {
        Failure::Exited { code }
    }
}

/// Remove ANSI escape sequences (Kanata colours its log levels).
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(strip_ansi).collect()
    }

    /// Real `kanata 1.12.0 --check` output for `(deflayer base a nosuchkey)`.
    const CHECK_OUTPUT: &str = "\
\x1b[0m08:00:03.7789 \x1b[34m[INFO] \x1b[0mkanata v1.12.0 starting
08:00:03.7789 [INFO] validating config only and exiting
08:00:03.7792 \x1b[31m[ERROR] \x1b[0m  × Error in configuration
   ╭─[bad.kbd:1:1]
 2 │ (deflayer base a nosuchkey)
   ·                  ────┬────
   ·                      ╰── Error here
   ╰────
  help: Unknown key/action: nosuchkey
";

    #[test]
    fn check_summary_from_help_line() {
        assert_eq!(
            classify_check(false, &lines(CHECK_OUTPUT)),
            Some(Failure::InvalidConfig {
                summary: "Unknown key/action: nosuchkey".into()
            })
        );
        assert_eq!(classify_check(true, &lines(CHECK_OUTPUT)), None);
    }

    #[test]
    fn check_summary_falls_back_to_error_line() {
        let out = lines("[INFO] x\n[ERROR] could not read file: No such file\n");
        assert_eq!(
            classify_check(false, &out),
            Some(Failure::InvalidConfig {
                summary: "could not read file: No such file".into()
            })
        );
    }

    #[test]
    fn exits() {
        let perm =
            lines("[ERROR] failed to open keyboard device(s): Permission denied (os error 13)");
        assert_eq!(classify_exit(Some(1), &perm, 7070), Failure::Permissions);
        let port = lines(
            "thread 'main' panicked at src/tcp_server.rs:141:56:\nTCP server starts: Os { code: 98, kind: AddrInUse, message: \"Address already in use\" }",
        );
        assert_eq!(
            classify_exit(None, &port, 7070),
            Failure::PortInUse { port: 7070 }
        );
        assert_eq!(
            classify_exit(Some(3), &lines("[INFO] bye"), 7070),
            Failure::Exited { code: Some(3) }
        );
        let missing = lines(
            "[ERROR] No keyboard devices were found. Try:\n1. Run 'kanata --list' to see available devices\n2. Check permissions: sudo usermod -a -G input $USER",
        );
        assert_eq!(classify_exit(Some(1), &missing, 7070), Failure::NoDevice);
    }

    #[test]
    fn retryable_and_notices() {
        assert!(!Failure::Permissions.retryable());
        assert!(
            !Failure::InvalidConfig {
                summary: String::new()
            }
            .retryable()
        );
        assert!(Failure::NoAnswer.retryable());
        assert_eq!(
            Failure::GaveUp(Box::new(Failure::NoAnswer)).notice(),
            "Kanata keeps stopping; see the log"
        );
    }

    #[test]
    fn strips_colours() {
        assert_eq!(strip_ansi("\x1b[34m[INFO]\x1b[0m hi"), "[INFO] hi");
    }
}
