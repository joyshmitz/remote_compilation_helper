//! Color and formatting definitions for CLI output.
//!
//! Provides consistent styling across the application with support for
//! color terminals, plain text, and ASCII fallbacks.

use colored::{Color, ColoredString, Colorize};

/// Unicode symbols with ASCII fallbacks for terminal compatibility.
#[derive(Debug, Clone, Copy)]
pub struct Symbols {
    pub success: &'static str,
    pub failure: &'static str,
    pub warning: &'static str,
    pub info: &'static str,
    pub bullet_filled: &'static str,
    pub bullet_empty: &'static str,
    pub bullet_half: &'static str,
    pub arrow_right: &'static str,
    pub spinner_frames: &'static [&'static str],
}

impl Symbols {
    /// Unicode symbols for terminals with full unicode support.
    pub const UNICODE: Self = Self {
        success: "✓",
        failure: "✗",
        warning: "⚠",
        info: "ℹ",
        bullet_filled: "●",
        bullet_empty: "○",
        bullet_half: "◐",
        arrow_right: "→",
        spinner_frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
    };

    /// ASCII fallbacks for terminals without unicode support.
    pub const ASCII: Self = Self {
        success: "[OK]",
        failure: "[FAIL]",
        warning: "[WARN]",
        info: "[INFO]",
        bullet_filled: "[*]",
        bullet_empty: "[ ]",
        bullet_half: "[~]",
        arrow_right: "->",
        spinner_frames: &["|", "/", "-", "\\"],
    };

    /// Select symbols based on unicode support.
    pub fn for_unicode(supports_unicode: bool) -> Self {
        if supports_unicode {
            Self::UNICODE
        } else {
            Self::ASCII
        }
    }
}

/// Semantic colors for different message types.
#[derive(Debug, Clone, Copy)]
pub struct SemanticColors {
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub muted: Color,
    pub highlight: Color,
    pub key: Color,
    pub value: Color,
}

impl Default for SemanticColors {
    fn default() -> Self {
        Self {
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            info: Color::Cyan,
            muted: Color::BrightBlack,
            highlight: Color::White,
            key: Color::Blue,
            value: Color::White,
        }
    }
}

/// Style configuration combining colors and symbols.
#[derive(Debug, Clone)]
pub struct Style {
    pub colors: SemanticColors,
    pub symbols: Symbols,
    pub colors_enabled: bool,
}

impl Style {
    /// Create a new style with colors and unicode support.
    pub fn new(colors_enabled: bool, supports_unicode: bool) -> Self {
        Self {
            colors: SemanticColors::default(),
            symbols: Symbols::for_unicode(supports_unicode),
            colors_enabled,
        }
    }

    /// Apply success styling to text.
    pub fn success(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.success)
        } else {
            text.normal()
        }
    }

    /// Apply error styling to text.
    pub fn error(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.error).bold()
        } else {
            text.normal()
        }
    }

    /// Apply warning styling to text.
    pub fn warning(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.warning)
        } else {
            text.normal()
        }
    }

    /// Apply info styling to text.
    pub fn info(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.info)
        } else {
            text.normal()
        }
    }

    /// Apply muted/dim styling to text.
    pub fn muted(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.muted)
        } else {
            text.normal()
        }
    }

    /// Apply highlight/bold styling to text.
    pub fn highlight(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.highlight).bold()
        } else {
            text.normal()
        }
    }

    /// Apply key styling for key-value pairs.
    pub fn key(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.key)
        } else {
            text.normal()
        }
    }

    /// Apply value styling for key-value pairs.
    pub fn value(&self, text: &str) -> ColoredString {
        if self.colors_enabled {
            text.color(self.colors.value)
        } else {
            text.normal()
        }
    }

    /// Format a success message with symbol and styling.
    pub fn format_success(&self, msg: &str) -> String {
        format!("{} {}", self.success(self.symbols.success), self.success(msg))
    }

    /// Format an error message with symbol and styling.
    pub fn format_error(&self, msg: &str) -> String {
        format!("{} {}", self.error(self.symbols.failure), self.error(msg))
    }

    /// Format a warning message with symbol and styling.
    pub fn format_warning(&self, msg: &str) -> String {
        format!("{} {}", self.warning(self.symbols.warning), self.warning(msg))
    }

    /// Format an info message with symbol and styling.
    pub fn format_info(&self, msg: &str) -> String {
        format!("{} {}", self.info(self.symbols.info), self.info(msg))
    }

    /// Format a key-value pair with proper alignment.
    pub fn format_key_value(&self, key: &str, value: &str, key_width: usize) -> String {
        format!(
            "{:width$} {} {}",
            self.key(key),
            self.muted(":"),
            self.value(value),
            width = key_width
        )
    }

    /// Format a section header.
    pub fn format_header(&self, title: &str) -> String {
        if self.colors_enabled {
            format!("\n{}\n{}", self.highlight(title), self.muted(&"─".repeat(title.len())))
        } else {
            format!("\n{}\n{}", title, "-".repeat(title.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbols_unicode() {
        let sym = Symbols::UNICODE;
        assert_eq!(sym.success, "✓");
        assert_eq!(sym.failure, "✗");
    }

    #[test]
    fn test_symbols_ascii() {
        let sym = Symbols::ASCII;
        assert_eq!(sym.success, "[OK]");
        assert_eq!(sym.failure, "[FAIL]");
    }

    #[test]
    fn test_symbols_selection() {
        let unicode_sym = Symbols::for_unicode(true);
        assert_eq!(unicode_sym.success, "✓");

        let ascii_sym = Symbols::for_unicode(false);
        assert_eq!(ascii_sym.success, "[OK]");
    }

    #[test]
    fn test_style_with_colors_disabled() {
        let style = Style::new(false, true);
        let styled = style.success("test");
        // When colors are disabled, the string should not contain escape codes
        let output = styled.to_string();
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn test_format_key_value() {
        let style = Style::new(false, true);
        let formatted = style.format_key_value("Status", "Healthy", 10);
        assert!(formatted.contains("Status"));
        assert!(formatted.contains("Healthy"));
        assert!(formatted.contains(":"));
    }

    #[test]
    fn test_format_header() {
        let style = Style::new(false, true);
        let header = style.format_header("Workers");
        assert!(header.contains("Workers"));
        assert!(header.contains("-------"));
    }
}
