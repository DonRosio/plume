//! User settings, stored in `%LOCALAPPDATA%\Plume\settings.txt` as `key=value` lines.
//!
//! The format is deliberately trivial (no parser dependency) and read defensively: unknown
//! keys, malformed values and absurd sizes are ignored, so a damaged file can never prevent
//! Plume from starting.

use std::fs;
use std::io;
use std::path::Path;

/// Last position and size of the window when it was neither minimised nor maximised
/// (physical pixels), plus whether it was maximised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Settings {
    /// Chromium low-end-device mode (`--eco`).
    pub eco: bool,
    /// InPrivate session, nothing written to disk (`--ephemeral`).
    pub ephemeral: bool,
    pub window: Option<Geometry>,
}

const MIN_SIZE: u32 = 400;
const MAX_SIZE: u32 = 16_384;
const MAX_COORD: i32 = 100_000;

impl Settings {
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        let (mut x, mut y, mut width, mut height, mut maximized) = (None, None, None, None, false);
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "eco" => settings.eco = value == "true",
                "ephemeral" => settings.ephemeral = value == "true",
                "window_x" => x = value.parse::<i32>().ok().filter(|v| v.abs() <= MAX_COORD),
                "window_y" => y = value.parse::<i32>().ok().filter(|v| v.abs() <= MAX_COORD),
                "window_width" => {
                    width = value
                        .parse::<u32>()
                        .ok()
                        .filter(|v| (MIN_SIZE..=MAX_SIZE).contains(v))
                }
                "window_height" => {
                    height = value
                        .parse::<u32>()
                        .ok()
                        .filter(|v| (MIN_SIZE..=MAX_SIZE).contains(v))
                }
                "window_maximized" => maximized = value == "true",
                _ => {}
            }
        }
        if let (Some(x), Some(y), Some(width), Some(height)) = (x, y, width, height) {
            settings.window = Some(Geometry {
                x,
                y,
                width,
                height,
                maximized,
            });
        }
        settings
    }

    pub fn serialize(&self) -> String {
        let mut text = format!("eco={}\nephemeral={}\n", self.eco, self.ephemeral);
        if let Some(g) = self.window {
            text.push_str(&format!(
                "window_x={}\nwindow_y={}\nwindow_width={}\nwindow_height={}\nwindow_maximized={}\n",
                g.x, g.y, g.width, g.height, g.maximized
            ));
        }
        text
    }

    pub fn load(path: &Path) -> Self {
        // Settings are a few hundred bytes: refuse anything larger instead of reading it.
        match fs::metadata(path) {
            Ok(meta) if meta.len() <= 4096 => fs::read_to_string(path)
                .map(|t| Self::parse(&t))
                .unwrap_or_default(),
            _ => Self::default(),
        }
    }

    /// Writes to a temporary file then renames it, so a crash never leaves a half-written file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, self.serialize())?;
        fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let settings = Settings {
            eco: true,
            ephemeral: false,
            window: Some(Geometry {
                x: -1200,
                y: 40,
                width: 1280,
                height: 900,
                maximized: true,
            }),
        };
        assert_eq!(Settings::parse(&settings.serialize()), settings);
    }

    #[test]
    fn damaged_files_are_harmless() {
        assert_eq!(Settings::parse(""), Settings::default());
        assert_eq!(
            Settings::parse("garbage\n\0\0\n=\neco=yes"),
            Settings::default()
        );
        // Absurd or incomplete geometry is dropped, the rest is kept.
        let s = Settings::parse(
            "eco=true\nwindow_x=10\nwindow_y=10\nwindow_width=99999999\nwindow_height=800",
        );
        assert!(s.eco);
        assert_eq!(s.window, None);
        assert_eq!(
            Settings::parse("window_x=1\nwindow_y=2\nwindow_width=900").window,
            None
        );
    }

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("plume-test-{}", std::process::id()));
        let path = dir.join("settings.txt");
        let settings = Settings {
            eco: false,
            ephemeral: true,
            window: None,
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        fs::write(&path, "x".repeat(10_000)).unwrap();
        assert_eq!(
            Settings::load(&path),
            Settings::default(),
            "oversized file ignored"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
