//! Application preferences: how Studio looks and behaves on this machine.
//!
//! These are global to Studio, NOT per-connection, and never reach a
//! database server. They live behind the Preferences dialog and persist
//! through `state::persist`.
//!
//! The look preferences apply through attributes on `.studio-root`
//! (`root_attrs`). The stylesheet reads them: `data-theme` swaps the color
//! tokens, `data-density` the control and row heights, `data-font-size` the
//! type scale, `data-editor-font` the monospace stack, and `data-motion`
//! turns animation off. The time zone applies where Studio prints a time.

use serde::{Deserialize, Serialize};

/// Color theme. `System` follows the OS preference, which is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Light, Theme::Dark, Theme::System];

    /// Value of the root `data-theme` attribute. The stylesheet keys the dark
    /// tokens on it, and on the OS preference when it is `system`.
    pub fn attr(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }

    /// The next theme in Light, Dark, System order.
    pub fn next(self) -> Theme {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::System,
            Theme::System => Theme::Light,
        }
    }
}

/// Control and row size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    pub const ALL: [Density; 2] = [Density::Comfortable, Density::Compact];

    pub fn attr(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Density::Comfortable => "Comfortable",
            Density::Compact => "Compact",
        }
    }
}

/// Base interface font size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSize {
    Px12,
    Px13,
    Px14,
    Px15,
    Px16,
}

impl FontSize {
    pub const ALL: [FontSize; 5] = [
        FontSize::Px12,
        FontSize::Px13,
        FontSize::Px14,
        FontSize::Px15,
        FontSize::Px16,
    ];

    pub fn px(self) -> u8 {
        match self {
            FontSize::Px12 => 12,
            FontSize::Px13 => 13,
            FontSize::Px14 => 14,
            FontSize::Px15 => 15,
            FontSize::Px16 => 16,
        }
    }

    /// Value of the root `data-font-size` attribute.
    pub fn attr(self) -> &'static str {
        match self {
            FontSize::Px12 => "12",
            FontSize::Px13 => "13",
            FontSize::Px14 => "14",
            FontSize::Px15 => "15",
            FontSize::Px16 => "16",
        }
    }
}

/// Monospace font for editors, grids, and code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorFont {
    JetBrainsMono,
    SfMono,
    Menlo,
    SystemMono,
}

impl EditorFont {
    pub const ALL: [EditorFont; 4] = [
        EditorFont::JetBrainsMono,
        EditorFont::SfMono,
        EditorFont::Menlo,
        EditorFont::SystemMono,
    ];

    /// Value of the root `data-editor-font` attribute.
    pub fn attr(self) -> &'static str {
        match self {
            EditorFont::JetBrainsMono => "jetbrains-mono",
            EditorFont::SfMono => "sf-mono",
            EditorFont::Menlo => "menlo",
            EditorFont::SystemMono => "system",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EditorFont::JetBrainsMono => "JetBrains Mono",
            EditorFont::SfMono => "SF Mono",
            EditorFont::Menlo => "Menlo",
            EditorFont::SystemMono => "System monospace",
        }
    }
}

/// Spaces per tab stop in editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabSize {
    Two,
    Four,
    Eight,
}

impl TabSize {
    pub const ALL: [TabSize; 3] = [TabSize::Two, TabSize::Four, TabSize::Eight];

    pub fn width(self) -> u8 {
        match self {
            TabSize::Two => 2,
            TabSize::Four => 4,
            TabSize::Eight => 8,
        }
    }
}

/// Time zones the Preferences dialog offers, as minutes east of UTC. A fixed
/// offset has no daylight saving, so a stamp never shifts by an hour
/// depending on the date.
pub const UTC_OFFSETS: [i16; 12] = [-480, -420, -360, -300, 0, 60, 120, 180, 330, 480, 540, 600];

/// Missing fields read as their defaults, so a file written by an older
/// Studio still loads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    pub density: Density,
    pub font_size: FontSize,
    pub editor_font: EditorFont,
    /// Turns off animation and transitions.
    pub reduce_motion: bool,
    /// Zone for every time Studio prints, in minutes east of UTC.
    pub utc_offset_minutes: i16,
    pub tab_size: TabSize,
    pub word_wrap: bool,
    /// Ask before a statement that deletes or drops data.
    pub confirm_destructive: bool,
    /// Whether anonymous usage telemetry is enabled.
    pub telemetry: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            density: Density::Compact,
            font_size: FontSize::Px14,
            editor_font: EditorFont::JetBrainsMono,
            reduce_motion: false,
            utc_offset_minutes: 0,
            tab_size: TabSize::Four,
            word_wrap: false,
            confirm_destructive: true,
            telemetry: false,
        }
    }
}

/// The attributes the root element carries for these preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootAttrs {
    pub theme: &'static str,
    pub density: &'static str,
    pub font_size: &'static str,
    pub editor_font: &'static str,
    pub motion: &'static str,
}

impl Preferences {
    pub fn root_attrs(&self) -> RootAttrs {
        RootAttrs {
            theme: self.theme.attr(),
            density: self.density.attr(),
            font_size: self.font_size.attr(),
            editor_font: self.editor_font.attr(),
            motion: if self.reduce_motion { "reduce" } else { "full" },
        }
    }

    /// `UTC` or `UTC+08:00`, the label of the chosen zone.
    pub fn zone_label(&self) -> String {
        crate::models::format::offset_label(self.utc_offset_minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_design() {
        let p = Preferences::default();
        assert_eq!(p.theme, Theme::System);
        assert_eq!(p.density, Density::Compact);
        assert_eq!(p.font_size.px(), 14);
        assert_eq!(p.editor_font.label(), "JetBrains Mono");
        assert!(!p.reduce_motion);
        assert_eq!(p.zone_label(), "UTC");
    }

    #[test]
    fn every_preference_reaches_the_root() {
        let p = Preferences {
            theme: Theme::Dark,
            density: Density::Comfortable,
            font_size: FontSize::Px16,
            editor_font: EditorFont::Menlo,
            reduce_motion: true,
            ..Preferences::default()
        };
        let a = p.root_attrs();
        assert_eq!(
            (a.theme, a.density, a.font_size, a.editor_font, a.motion),
            ("dark", "comfortable", "16", "menlo", "reduce")
        );
    }

    /// Two options with one attribute value would style as one.
    #[test]
    fn attributes_are_unique_per_set() {
        fn unique(values: Vec<&str>) {
            let set: std::collections::HashSet<_> = values.iter().collect();
            assert_eq!(set.len(), values.len(), "{values:?}");
        }
        unique(Theme::ALL.iter().map(|t| t.attr()).collect());
        unique(Density::ALL.iter().map(|t| t.attr()).collect());
        unique(FontSize::ALL.iter().map(|t| t.attr()).collect());
        unique(EditorFont::ALL.iter().map(|t| t.attr()).collect());
    }

    #[test]
    fn theme_cycles_through_all_three() {
        let mut t = Theme::Light;
        let mut seen = vec![t];
        for _ in 0..3 {
            t = t.next();
            seen.push(t);
        }
        assert_eq!(
            seen,
            [Theme::Light, Theme::Dark, Theme::System, Theme::Light]
        );
    }

    /// A file from an older Studio has fewer fields and holds a field this
    /// Studio dropped. It must still load.
    #[test]
    fn an_older_file_loads_with_defaults_for_the_new_fields() {
        let old = r#"{"theme":"Dark","mono_font":"ui-monospace","telemetry":true}"#;
        let p: Preferences = sonic_rs::from_str(old).expect("old file loads");
        assert_eq!(p.theme, Theme::Dark);
        assert!(p.telemetry);
        assert_eq!(p.density, Density::Compact);
    }

    #[test]
    fn preferences_round_trip_through_json() {
        let p = Preferences {
            utc_offset_minutes: 480,
            word_wrap: true,
            ..Preferences::default()
        };
        let json = sonic_rs::to_string(&p).expect("serializes");
        let back: Preferences = sonic_rs::from_str(&json).expect("parses");
        assert_eq!(back, p);
        assert_eq!(back.zone_label(), "UTC+08:00");
    }

    #[test]
    fn the_default_zone_is_offered() {
        assert!(UTC_OFFSETS.contains(&Preferences::default().utc_offset_minutes));
    }
}
