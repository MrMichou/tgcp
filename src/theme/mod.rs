//! Theme System
//!
//! Customizable color themes for tgcp, inspired by k9s.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Validate theme name to prevent path traversal attacks
/// Theme names can only contain alphanumeric characters, hyphens, and underscores
fn validate_theme_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }

    // Must not start with a dot or hyphen
    if name.starts_with('.') || name.starts_with('-') {
        return false;
    }

    // Only allow safe characters
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// RGB color as [r, g, b]
pub type Rgb = [u8; 3];

/// Complete theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme name
    #[serde(default = "default_name")]
    pub name: String,

    /// Base colors
    #[serde(default)]
    pub base: BaseColors,

    /// Table colors
    #[serde(default)]
    pub table: TableColors,

    /// Status colors (for resource states)
    #[serde(default)]
    pub status: StatusColors,

    /// Dialog colors
    #[serde(default)]
    pub dialog: DialogColors,

    /// Syntax highlighting (for JSON view)
    #[serde(default)]
    pub syntax: SyntaxColors,
}

fn default_name() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseColors {
    /// Main background
    #[serde(default = "default_bg")]
    pub background: Rgb,
    /// Main foreground/text
    #[serde(default = "default_fg")]
    pub foreground: Rgb,
    /// Border color
    #[serde(default = "default_border")]
    pub border: Rgb,
    /// Accent color (titles, highlights)
    #[serde(default = "default_accent")]
    pub accent: Rgb,
    /// Muted/secondary text
    #[serde(default = "default_muted")]
    pub muted: Rgb,
    /// Error color
    #[serde(default = "default_error")]
    pub error: Rgb,
    /// Warning color
    #[serde(default = "default_warning")]
    pub warning: Rgb,
    /// Success color
    #[serde(default = "default_success")]
    pub success: Rgb,
}

fn default_bg() -> Rgb {
    [0, 0, 0]
}
fn default_fg() -> Rgb {
    [255, 255, 255]
}
fn default_border() -> Rgb {
    [128, 128, 128]
}
fn default_accent() -> Rgb {
    [0, 255, 255]
}
fn default_muted() -> Rgb {
    [128, 128, 128]
}
fn default_error() -> Rgb {
    [255, 85, 85]
}
fn default_warning() -> Rgb {
    [255, 255, 85]
}
fn default_success() -> Rgb {
    [85, 255, 85]
}

impl Default for BaseColors {
    fn default() -> Self {
        Self {
            background: default_bg(),
            foreground: default_fg(),
            border: default_border(),
            accent: default_accent(),
            muted: default_muted(),
            error: default_error(),
            warning: default_warning(),
            success: default_success(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColors {
    /// Header text color
    #[serde(default = "default_header")]
    pub header: Rgb,
    /// Selected row background
    #[serde(default = "default_selected_bg")]
    pub selected_bg: Rgb,
    /// Selected row foreground
    #[serde(default = "default_selected_fg")]
    pub selected_fg: Rgb,
}

fn default_header() -> Rgb {
    [255, 255, 0]
}
fn default_selected_bg() -> Rgb {
    [68, 68, 68]
}
fn default_selected_fg() -> Rgb {
    [255, 255, 255]
}

impl Default for TableColors {
    fn default() -> Self {
        Self {
            header: default_header(),
            selected_bg: default_selected_bg(),
            selected_fg: default_selected_fg(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusColors {
    /// Running/active states
    #[serde(default = "default_running")]
    pub running: Rgb,
    /// Stopped/terminated states
    #[serde(default = "default_stopped")]
    pub stopped: Rgb,
    /// Pending/transitional states
    #[serde(default = "default_pending")]
    pub pending: Rgb,
    /// Error/failed states
    #[serde(default = "default_failed")]
    pub failed: Rgb,
    /// Unknown/other states
    #[serde(default = "default_unknown")]
    pub unknown: Rgb,
}

fn default_running() -> Rgb {
    [85, 255, 85]
}
fn default_stopped() -> Rgb {
    [128, 128, 128]
}
fn default_pending() -> Rgb {
    [255, 255, 85]
}
fn default_failed() -> Rgb {
    [255, 85, 85]
}
fn default_unknown() -> Rgb {
    [128, 128, 128]
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            running: default_running(),
            stopped: default_stopped(),
            pending: default_pending(),
            failed: default_failed(),
            unknown: default_unknown(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogColors {
    /// Dialog background
    #[serde(default = "default_dialog_bg")]
    pub background: Rgb,
    /// Dialog border
    #[serde(default = "default_dialog_border")]
    pub border: Rgb,
    /// Button background
    #[serde(default = "default_button_bg")]
    pub button_bg: Rgb,
    /// Selected button background
    #[serde(default = "default_button_selected")]
    pub button_selected: Rgb,
    /// Destructive action color
    #[serde(default = "default_destructive")]
    pub destructive: Rgb,
}

fn default_dialog_bg() -> Rgb {
    [40, 40, 40]
}
fn default_dialog_border() -> Rgb {
    [128, 128, 128]
}
fn default_button_bg() -> Rgb {
    [68, 68, 68]
}
fn default_button_selected() -> Rgb {
    [0, 128, 255]
}
fn default_destructive() -> Rgb {
    [255, 85, 85]
}

impl Default for DialogColors {
    fn default() -> Self {
        Self {
            background: default_dialog_bg(),
            border: default_dialog_border(),
            button_bg: default_button_bg(),
            button_selected: default_button_selected(),
            destructive: default_destructive(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxColors {
    /// JSON keys
    #[serde(default = "default_syntax_key")]
    pub key: Rgb,
    /// String values
    #[serde(default = "default_syntax_string")]
    pub string: Rgb,
    /// Number values
    #[serde(default = "default_syntax_number")]
    pub number: Rgb,
    /// Boolean values
    #[serde(default = "default_syntax_boolean")]
    pub boolean: Rgb,
    /// Null values
    #[serde(default = "default_syntax_null")]
    pub null: Rgb,
    /// Brackets/braces
    #[serde(default = "default_syntax_bracket")]
    pub bracket: Rgb,
}

fn default_syntax_key() -> Rgb {
    [0, 255, 255]
}
fn default_syntax_string() -> Rgb {
    [85, 255, 85]
}
fn default_syntax_number() -> Rgb {
    [135, 175, 255]
}
fn default_syntax_boolean() -> Rgb {
    [255, 85, 255]
}
fn default_syntax_null() -> Rgb {
    [128, 128, 128]
}
fn default_syntax_bracket() -> Rgb {
    [255, 255, 0]
}

impl Default for SyntaxColors {
    fn default() -> Self {
        Self {
            key: default_syntax_key(),
            string: default_syntax_string(),
            number: default_syntax_number(),
            boolean: default_syntax_boolean(),
            null: default_syntax_null(),
            bracket: default_syntax_bracket(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            base: BaseColors::default(),
            table: TableColors::default(),
            status: StatusColors::default(),
            dialog: DialogColors::default(),
            syntax: SyntaxColors::default(),
        }
    }
}

impl Theme {
    /// Get built-in theme by name
    pub fn builtin(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "default" => Some(Self::default()),
            "dracula" => Some(Self::dracula()),
            "monokai" => Some(Self::monokai()),
            "nord" => Some(Self::nord()),
            "gruvbox" => Some(Self::gruvbox()),
            "solarized" | "solarized-dark" => Some(Self::solarized_dark()),
            "production" | "prod" => Some(Self::production()),
            _ => None,
        }
    }

    /// Dracula theme
    pub fn dracula() -> Self {
        Self {
            name: "dracula".to_string(),
            base: BaseColors {
                background: [40, 42, 54],
                foreground: [248, 248, 242],
                border: [68, 71, 90],
                accent: [139, 233, 253],
                muted: [98, 114, 164],
                error: [255, 85, 85],
                warning: [241, 250, 140],
                success: [80, 250, 123],
            },
            table: TableColors {
                header: [189, 147, 249],
                selected_bg: [68, 71, 90],
                selected_fg: [248, 248, 242],
            },
            status: StatusColors {
                running: [80, 250, 123],
                stopped: [98, 114, 164],
                pending: [241, 250, 140],
                failed: [255, 85, 85],
                unknown: [98, 114, 164],
            },
            dialog: DialogColors {
                background: [40, 42, 54],
                border: [189, 147, 249],
                button_bg: [68, 71, 90],
                button_selected: [139, 233, 253],
                destructive: [255, 85, 85],
            },
            syntax: SyntaxColors {
                key: [139, 233, 253],
                string: [80, 250, 123],
                number: [189, 147, 249],
                boolean: [255, 184, 108],
                null: [98, 114, 164],
                bracket: [241, 250, 140],
            },
        }
    }

    /// Monokai theme
    pub fn monokai() -> Self {
        Self {
            name: "monokai".to_string(),
            base: BaseColors {
                background: [39, 40, 34],
                foreground: [248, 248, 242],
                border: [117, 113, 94],
                accent: [102, 217, 239],
                muted: [117, 113, 94],
                error: [249, 38, 114],
                warning: [230, 219, 116],
                success: [166, 226, 46],
            },
            table: TableColors {
                header: [249, 38, 114],
                selected_bg: [73, 72, 62],
                selected_fg: [248, 248, 242],
            },
            status: StatusColors {
                running: [166, 226, 46],
                stopped: [117, 113, 94],
                pending: [230, 219, 116],
                failed: [249, 38, 114],
                unknown: [117, 113, 94],
            },
            dialog: DialogColors {
                background: [39, 40, 34],
                border: [249, 38, 114],
                button_bg: [73, 72, 62],
                button_selected: [102, 217, 239],
                destructive: [249, 38, 114],
            },
            syntax: SyntaxColors {
                key: [102, 217, 239],
                string: [230, 219, 116],
                number: [174, 129, 255],
                boolean: [174, 129, 255],
                null: [117, 113, 94],
                bracket: [248, 248, 242],
            },
        }
    }

    /// Nord theme
    pub fn nord() -> Self {
        Self {
            name: "nord".to_string(),
            base: BaseColors {
                background: [46, 52, 64],
                foreground: [236, 239, 244],
                border: [76, 86, 106],
                accent: [136, 192, 208],
                muted: [76, 86, 106],
                error: [191, 97, 106],
                warning: [235, 203, 139],
                success: [163, 190, 140],
            },
            table: TableColors {
                header: [129, 161, 193],
                selected_bg: [67, 76, 94],
                selected_fg: [236, 239, 244],
            },
            status: StatusColors {
                running: [163, 190, 140],
                stopped: [76, 86, 106],
                pending: [235, 203, 139],
                failed: [191, 97, 106],
                unknown: [76, 86, 106],
            },
            dialog: DialogColors {
                background: [59, 66, 82],
                border: [136, 192, 208],
                button_bg: [67, 76, 94],
                button_selected: [136, 192, 208],
                destructive: [191, 97, 106],
            },
            syntax: SyntaxColors {
                key: [136, 192, 208],
                string: [163, 190, 140],
                number: [180, 142, 173],
                boolean: [180, 142, 173],
                null: [76, 86, 106],
                bracket: [235, 203, 139],
            },
        }
    }

    /// Gruvbox theme
    pub fn gruvbox() -> Self {
        Self {
            name: "gruvbox".to_string(),
            base: BaseColors {
                background: [40, 40, 40],
                foreground: [235, 219, 178],
                border: [102, 92, 84],
                accent: [131, 165, 152],
                muted: [146, 131, 116],
                error: [251, 73, 52],
                warning: [250, 189, 47],
                success: [184, 187, 38],
            },
            table: TableColors {
                header: [254, 128, 25],
                selected_bg: [60, 56, 54],
                selected_fg: [235, 219, 178],
            },
            status: StatusColors {
                running: [184, 187, 38],
                stopped: [146, 131, 116],
                pending: [250, 189, 47],
                failed: [251, 73, 52],
                unknown: [146, 131, 116],
            },
            dialog: DialogColors {
                background: [50, 48, 47],
                border: [131, 165, 152],
                button_bg: [60, 56, 54],
                button_selected: [131, 165, 152],
                destructive: [251, 73, 52],
            },
            syntax: SyntaxColors {
                key: [131, 165, 152],
                string: [184, 187, 38],
                number: [211, 134, 155],
                boolean: [211, 134, 155],
                null: [146, 131, 116],
                bracket: [250, 189, 47],
            },
        }
    }

    /// Solarized Dark theme
    pub fn solarized_dark() -> Self {
        Self {
            name: "solarized".to_string(),
            base: BaseColors {
                background: [0, 43, 54],
                foreground: [131, 148, 150],
                border: [88, 110, 117],
                accent: [38, 139, 210],
                muted: [88, 110, 117],
                error: [220, 50, 47],
                warning: [181, 137, 0],
                success: [133, 153, 0],
            },
            table: TableColors {
                header: [181, 137, 0],
                selected_bg: [7, 54, 66],
                selected_fg: [147, 161, 161],
            },
            status: StatusColors {
                running: [133, 153, 0],
                stopped: [88, 110, 117],
                pending: [181, 137, 0],
                failed: [220, 50, 47],
                unknown: [88, 110, 117],
            },
            dialog: DialogColors {
                background: [0, 43, 54],
                border: [38, 139, 210],
                button_bg: [7, 54, 66],
                button_selected: [38, 139, 210],
                destructive: [220, 50, 47],
            },
            syntax: SyntaxColors {
                key: [38, 139, 210],
                string: [42, 161, 152],
                number: [108, 113, 196],
                boolean: [108, 113, 196],
                null: [88, 110, 117],
                bracket: [181, 137, 0],
            },
        }
    }

    /// Production environment theme (red tones to warn user)
    pub fn production() -> Self {
        Self {
            name: "production".to_string(),
            base: BaseColors {
                background: [30, 15, 15],
                foreground: [255, 200, 200],
                border: [139, 69, 69],
                accent: [255, 100, 100],
                muted: [139, 100, 100],
                error: [255, 50, 50],
                warning: [255, 200, 100],
                success: [100, 200, 100],
            },
            table: TableColors {
                header: [255, 100, 100],
                selected_bg: [80, 30, 30],
                selected_fg: [255, 220, 220],
            },
            status: StatusColors {
                running: [100, 200, 100],
                stopped: [139, 100, 100],
                pending: [255, 200, 100],
                failed: [255, 50, 50],
                unknown: [139, 100, 100],
            },
            dialog: DialogColors {
                background: [50, 20, 20],
                border: [255, 100, 100],
                button_bg: [80, 30, 30],
                button_selected: [255, 100, 100],
                destructive: [255, 50, 50],
            },
            syntax: SyntaxColors {
                key: [255, 150, 150],
                string: [150, 200, 150],
                number: [200, 150, 255],
                boolean: [200, 150, 255],
                null: [139, 100, 100],
                bracket: [255, 200, 100],
            },
        }
    }

    /// Load theme from file
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = serde_yml::from_str(&content)?;
        Ok(theme)
    }
}

/// Theme manager for loading and caching themes
pub struct ThemeManager {
    /// Currently active theme
    current: Theme,
}

impl ThemeManager {
    /// Create new theme manager with default theme
    pub fn new() -> Self {
        Self {
            current: Theme::default(),
        }
    }

    /// Load theme manager from config
    pub fn load() -> Self {
        let mut manager = Self::new();

        // Try to load theme config
        if let Some(config_dir) = dirs::config_dir() {
            let theme_config = config_dir.join("tgcp").join("theme.yaml");
            if theme_config.exists() {
                if let Ok(theme) = Theme::load_from_file(&theme_config) {
                    manager.current = theme;
                }
            }
        }

        // Check environment variable
        if let Ok(theme_name) = std::env::var("TGCP_THEME") {
            // Security: Validate theme name to prevent path traversal
            if !validate_theme_name(&theme_name) {
                tracing::warn!("Invalid theme name in TGCP_THEME: contains unsafe characters");
            } else if let Some(theme) = Theme::builtin(&theme_name) {
                manager.current = theme;
            } else {
                // Try loading from skins directory
                if let Some(config_dir) = dirs::config_dir() {
                    let theme_path = config_dir
                        .join("tgcp")
                        .join("skins")
                        .join(format!("{}.yaml", theme_name));
                    if let Ok(theme) = Theme::load_from_file(&theme_path) {
                        manager.current = theme;
                    }
                }
            }
        }

        manager
    }

    /// Set theme by name (builtin or custom)
    /// Security: Validates theme name to prevent path traversal
    pub fn set_theme(&mut self, name: &str) -> bool {
        // Security: Validate theme name first
        if !validate_theme_name(name) {
            tracing::warn!("Invalid theme name: '{}' contains unsafe characters", name);
            return false;
        }

        if let Some(theme) = Theme::builtin(name) {
            self.current = theme;
            true
        } else if let Some(config_dir) = dirs::config_dir() {
            let theme_path = config_dir
                .join("tgcp")
                .join("skins")
                .join(format!("{}.yaml", name));
            if let Ok(theme) = Theme::load_from_file(&theme_path) {
                self.current = theme;
                return true;
            }
            false
        } else {
            false
        }
    }

    /// List available themes
    pub fn list_available() -> Vec<String> {
        let mut themes = vec![
            "default".to_string(),
            "dracula".to_string(),
            "monokai".to_string(),
            "nord".to_string(),
            "gruvbox".to_string(),
            "solarized".to_string(),
            "production".to_string(),
        ];

        // Add custom themes from skins directory
        if let Some(config_dir) = dirs::config_dir() {
            let skins_dir = config_dir.join("tgcp").join("skins");
            if let Ok(entries) = std::fs::read_dir(skins_dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.path().file_stem() {
                        if let Some(name_str) = name.to_str() {
                            if !themes.contains(&name_str.to_string()) {
                                themes.push(name_str.to_string());
                            }
                        }
                    }
                }
            }
        }

        themes
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}
