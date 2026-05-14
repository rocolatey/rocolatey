use std::io::IsTerminal;
use std::sync::atomic::{AtomicU8, Ordering};
use owo_colors::OwoColorize;

/// Controls whether ANSI color codes are emitted.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Emit colors only when stdout is a TTY that supports them.
    Auto,
    /// Always emit colors.
    Always,
    /// Never emit colors.
    Never,
}

impl ColorMode {
    /// Returns `true` when colors should be written to stdout.
    pub fn is_enabled(self) -> bool {
        self.is_enabled_for_tty(std::io::stdout().is_terminal())
    }

    /// Resolve mode behavior for a known stdout TTY state.
    fn is_enabled_for_tty(self, stdout_is_tty: bool) -> bool {
        match self {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => stdout_is_tty,
        }
    }
}

// ---------------------------------------------------------------------------
// Process-wide active mode (set at startup by main.rs)
// ---------------------------------------------------------------------------

static ACTIVE_MODE: AtomicU8 = AtomicU8::new(ColorMode::Auto as u8);

impl ColorMode {
    fn from_repr(value: u8) -> Self {
        match value {
            x if x == ColorMode::Always as u8 => ColorMode::Always,
            x if x == ColorMode::Never as u8 => ColorMode::Never,
            _ => ColorMode::Auto,
        }
    }
}

/// Set the process-wide color mode.  Call this in `main` before
/// dispatching any subcommand.
pub fn init(mode: ColorMode) {
    ACTIVE_MODE.store(mode as u8, Ordering::Release);
}

/// Return the process-wide color mode set by [`init`].
/// Command handlers call this to respect the user's `--color` flag.
pub fn current() -> ColorMode {
    ColorMode::from_repr(ACTIVE_MODE.load(Ordering::Acquire))
}

// ---------------------------------------------------------------------------
// Semantic style helpers
//
// Each function returns an owned String.  When colors are disabled the input
// is returned unchanged; when enabled it is wrapped with ANSI escape codes.
// ---------------------------------------------------------------------------

/// Section headers (cyan bold).
pub fn header(text: &str, mode: ColorMode) -> String {
    if mode.is_enabled() {
        format!("{}", text.cyan().bold())
    } else {
        text.to_owned()
    }
}

/// Informational / secondary lines (bright blue).
pub fn info(text: &str, mode: ColorMode) -> String {
    if mode.is_enabled() {
        format!("{}", text.bright_blue())
    } else {
        text.to_owned()
    }
}

/// Package result rows (default fg — bold so they stand out).
pub fn package(text: &str, mode: ColorMode) -> String {
    if mode.is_enabled() {
        format!("{}", text.white())
    } else {
        text.to_owned()
    }
}

/// Warning lines (yellow).
pub fn warning(text: &str, mode: ColorMode) -> String {
    if mode.is_enabled() {
        format!("{}", text.yellow())
    } else {
        text.to_owned()
    }
}

/// Error lines (red bold).
pub fn error(text: &str, mode: ColorMode) -> String {
    if mode.is_enabled() {
        format!("{}", text.red().bold())
    } else {
        text.to_owned()
    }
}

/// A stable (non-pre-release) version token (green).
pub fn version_stable(text: &str, mode: ColorMode) -> String {
    if mode.is_enabled() {
        format!("{}", text.green())
    } else {
        text.to_owned()
    }
}

/// A pre-release version token (magenta).
pub fn version_prerelease(text: &str, mode: ColorMode) -> String {
    if mode.is_enabled() {
        format!("{}", text.magenta())
    } else {
        text.to_owned()
    }
}

/// Tree dependency family lines. Color rotates by group index.
pub fn dependency_group(text: &str, group_idx: usize, mode: ColorMode) -> String {
    if !mode.is_enabled() {
        return text.to_owned();
    }

    match group_idx % 6 {
        0 => format!("{}", text.bright_cyan()),
        1 => format!("{}", text.bright_blue()),
        2 => format!("{}", text.bright_green()),
        3 => format!("{}", text.bright_yellow()),
        4 => format!("{}", text.bright_magenta()),
        _ => format!("{}", text.bright_white()),
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_mode_returns_plain_text() {
        assert_eq!(header("Hello", ColorMode::Never), "Hello");
        assert_eq!(info("Hello", ColorMode::Never), "Hello");
        assert_eq!(package("Hello", ColorMode::Never), "Hello");
        assert_eq!(warning("Hello", ColorMode::Never), "Hello");
        assert_eq!(error("Hello", ColorMode::Never), "Hello");
        assert_eq!(version_stable("1.2.3", ColorMode::Never), "1.2.3");
        assert_eq!(version_prerelease("1.2.3-beta", ColorMode::Never), "1.2.3-beta");
    }

    #[test]
    fn always_mode_wraps_with_ansi() {
        let out = header("Hello", ColorMode::Always);
        assert!(out.contains("Hello"));
        assert!(out.contains('\x1b'), "expected ANSI escape in: {:?}", out);

        let stable = version_stable("1.2.3", ColorMode::Always);
        assert!(stable.contains('\x1b'));

        let pre = version_prerelease("1.2.3-beta", ColorMode::Always);
        assert!(pre.contains('\x1b'));

        let group = dependency_group("tree", 0, ColorMode::Always);
        assert!(group.contains('\x1b'));
    }

    #[test]
    fn is_enabled_always() {
        assert!(ColorMode::Always.is_enabled());
    }

    #[test]
    fn is_enabled_never() {
        assert!(!ColorMode::Never.is_enabled());
    }

    #[test]
    fn auto_mode_enabled_when_stdout_is_tty() {
        assert!(ColorMode::Auto.is_enabled_for_tty(true));
    }

    #[test]
    fn auto_mode_disabled_when_stdout_is_not_tty() {
        assert!(!ColorMode::Auto.is_enabled_for_tty(false));
    }

    #[test]
    fn always_mode_enabled_independent_of_tty() {
        assert!(ColorMode::Always.is_enabled_for_tty(true));
        assert!(ColorMode::Always.is_enabled_for_tty(false));
    }

    #[test]
    fn never_mode_disabled_independent_of_tty() {
        assert!(!ColorMode::Never.is_enabled_for_tty(true));
        assert!(!ColorMode::Never.is_enabled_for_tty(false));
    }

    #[test]
    fn current_mode_is_visible_across_threads() {
        init(ColorMode::Always);

        let mode = std::thread::spawn(current).join().unwrap();

        assert_eq!(mode, ColorMode::Always);
        init(ColorMode::Auto);
    }
}
