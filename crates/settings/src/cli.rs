//! Command-line overrides: settings for one run that never reach the file.

use std::fmt;
use std::path::PathBuf;

use crate::{Appearance, Edge, Field, MARGIN_RANGE, OPACITY_RANGE, SCALE_RANGE, in_range};

/// Settings given on the command line. `None` means "use the file's value".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    pub edge: Option<Edge>,
    pub margin: Option<u32>,
    pub opacity: Option<u8>,
    pub scale: Option<u16>,
    /// `Some(None)` overrides to the primary screen.
    pub screen: Option<Option<String>>,
    /// Read and save this settings file instead of the default one.
    pub settings_path: Option<PathBuf>,
}

/// A bad command-line value; the app prints it and exits with status 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub flag: &'static str,
    pub value: String,
    pub reason: String,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid --{} value '{}': {}",
            self.flag, self.value, self.reason
        )
    }
}

impl std::error::Error for CliError {}

impl Overrides {
    /// Parse the overlay's flags from `args` (without the program name).
    ///
    /// Other arguments, such as `--connect` or Qt's own `-platform`, are ignored.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, CliError> {
        let mut out = Self::default();
        let mut it = args.into_iter();
        while let Some(arg) = it.next() {
            let Some(flag) = arg.strip_prefix("--") else {
                continue;
            };
            let flag: &'static str = match flag {
                "settings" => "settings",
                "edge" => "edge",
                "margin" => "margin",
                "opacity" => "opacity",
                "scale" => "scale",
                "screen" => "screen",
                _ => continue,
            };
            let value = it.next().ok_or_else(|| CliError {
                flag,
                value: String::new(),
                reason: "missing value".to_owned(),
            })?;
            let error = |reason: String| CliError {
                flag,
                value: value.clone(),
                reason,
            };
            match flag {
                "settings" => out.settings_path = Some(PathBuf::from(&value)),
                "edge" => out.edge = Some(value.parse().map_err(error)?),
                "margin" => out.margin = Some(number(&value, &MARGIN_RANGE).map_err(error)?),
                "opacity" => out.opacity = Some(number(&value, &OPACITY_RANGE).map_err(error)?),
                "scale" => out.scale = Some(number(&value, &SCALE_RANGE).map_err(error)?),
                _ => out.screen = Some(crate::screen_name(&value)),
            }
        }
        Ok(out)
    }

    /// A board's appearance with these overrides applied.
    pub fn effective(&self, file: &Appearance) -> Appearance {
        Appearance {
            edge: self.edge.unwrap_or(file.edge),
            margin: self.margin.unwrap_or(file.margin),
            opacity: self.opacity.unwrap_or(file.opacity),
            scale: self.scale.unwrap_or(file.scale),
            screen: self.screen.clone().unwrap_or_else(|| file.screen.clone()),
        }
    }

    /// Drop one override, e.g. after the user edits that field in the settings window.
    pub fn clear(&mut self, field: Field) {
        match field {
            Field::Edge => self.edge = None,
            Field::Margin => self.margin = None,
            Field::Opacity => self.opacity = None,
            Field::Scale => self.scale = None,
            Field::Screen => self.screen = None,
        }
    }
}

fn number<T>(value: &str, range: &std::ops::RangeInclusive<T>) -> Result<T, String>
where
    T: std::str::FromStr + PartialOrd + fmt::Display,
{
    let n = value
        .parse::<T>()
        .map_err(|_| format!("expected a whole number {}–{}", range.start(), range.end()))?;
    in_range(n, range)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Overrides, CliError> {
        Overrides::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_every_flag() {
        let o = parse(&[
            "--settings",
            "/tmp/s.json",
            "--edge",
            "top-right",
            "--margin",
            "0",
            "--opacity",
            "50",
            "--scale",
            "300",
            "--screen",
            "DP-1",
        ])
        .unwrap();
        assert_eq!(
            o,
            Overrides {
                edge: Some(Edge::TopRight),
                margin: Some(0),
                opacity: Some(50),
                scale: Some(300),
                screen: Some(Some("DP-1".to_owned())),
                settings_path: Some(PathBuf::from("/tmp/s.json")),
            }
        );
    }

    #[test]
    fn screen_primary_means_none() {
        assert_eq!(parse(&["--screen", "primary"]).unwrap().screen, Some(None));
    }

    #[test]
    fn ignores_other_arguments() {
        let o = parse(&[
            "-platform",
            "wayland",
            "--connect",
            "x:1",
            "--layout",
            "a.json",
        ])
        .unwrap();
        assert_eq!(o, Overrides::default());
    }

    #[test]
    fn rejects_bad_values() {
        for (args, flag) in [
            (&["--scale", "9000"][..], "scale"),
            (&["--opacity", "abc"][..], "opacity"),
            (&["--opacity", "5"][..], "opacity"),
            (&["--edge", "diagonal"][..], "edge"),
            (&["--margin"][..], "margin"),
        ] {
            let err = parse(args).unwrap_err();
            assert_eq!(err.flag, flag, "{args:?}");
        }
        assert_eq!(
            parse(&["--scale", "9000"]).unwrap_err().to_string(),
            "invalid --scale value '9000': 9000 is outside 50–300"
        );
    }

    #[test]
    fn overrides_win_and_can_be_cleared() {
        let file = Appearance {
            opacity: 50,
            ..Appearance::default()
        };
        let mut o = parse(&["--opacity", "100", "--edge", "left"]).unwrap();
        let eff = o.effective(&file);
        assert_eq!((eff.opacity, eff.edge, eff.scale), (100, Edge::Left, 100));

        o.clear(Field::Opacity);
        let eff = o.effective(&file);
        assert_eq!((eff.opacity, eff.edge), (50, Edge::Left));
    }
}
