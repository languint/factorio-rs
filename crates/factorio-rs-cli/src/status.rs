//! Cargo-style status lines for the CLI.
//!
//! Status words are right-aligned in a fixed column so output scans like:
//!
//! ```text
//!     Finished transpile [debug] -> dist/ (12 files) in 1.30s
//!      Created factorio-rs project
//! ```

use std::fmt::Display;
use std::io::{IsTerminal, Write, stderr, stdout};
use std::path::Path;
use std::time::Duration;

use yansi::{Paint, Style};

const STATUS_WIDTH: usize = 12;

/// Semantic status kinds (Cargo-adjacent vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Checking,
    Finished,
    Created,
    Packaged,
    Installed,
    Opened,
    Added,
    Running,
    Note,
    Error,
}

impl Status {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Checking => "Checking",
            Self::Finished => "Finished",
            Self::Created => "Created",
            Self::Packaged => "Packaged",
            Self::Installed => "Installed",
            Self::Opened => "Opened",
            Self::Added => "Added",
            Self::Running => "Running",
            Self::Note => "Note",
            Self::Error => "error",
        }
    }

    const fn style(self) -> Style {
        match self {
            Self::Error => Style::new().red().bold(),
            Self::Note | Self::Running | Self::Checking => Style::new().cyan().bold(),
            Self::Finished
            | Self::Created
            | Self::Packaged
            | Self::Installed
            | Self::Opened
            | Self::Added => Style::new().green().bold(),
        }
    }
}

/// Whether ANSI color should be used on stdout.
///
/// Honors `NO_COLOR`, `CARGO_TERM_COLOR` (`always`/`never`), and `CLICOLOR_FORCE`
/// so Bacon (which sets `CARGO_TERM_COLOR=always` and pipes output) still gets color.
#[must_use]
pub fn color_stdout() -> bool {
    color_for_stream(stdout().is_terminal())
}

/// Whether ANSI color should be used on stderr.
#[must_use]
pub fn color_stderr() -> bool {
    color_for_stream(stderr().is_terminal())
}

fn color_for_stream(is_tty: bool) -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    match std::env::var("CARGO_TERM_COLOR").ok().as_deref() {
        Some("always") => return true,
        Some("never") => return false,
        _ => {}
    }
    if std::env::var_os("CLICOLOR_FORCE").is_some() {
        return true;
    }
    is_tty
}

/// Print a status line to stdout.
pub fn status(kind: Status, message: impl Display) {
    let _ = writeln!(stdout(), "{}", format_status(kind, message, color_stdout()));
}

/// Print a status line to stderr (errors / notes that must not mix with progress).
pub fn status_err(kind: Status, message: impl Display) {
    let _ = writeln!(stderr(), "{}", format_status(kind, message, color_stderr()));
}

/// Format a status line without writing it (for embedding above a progress bar).
#[must_use]
pub fn format_status(kind: Status, message: impl Display, color: bool) -> String {
    let word = format!("{:>width$}", kind.as_str(), width = STATUS_WIDTH);
    let styled = if color {
        format!("{}", word.paint(kind.style()))
    } else {
        word
    };
    format!("{styled} {message}")
}

/// Dim secondary detail (phase timings, hints).
#[must_use]
pub fn dim(text: impl Display, color: bool) -> String {
    let s = text.to_string();
    if color {
        format!("{}", s.paint(Style::new().dim()))
    } else {
        s
    }
}

/// Bold green / red / cyan helpers for test reports.
#[must_use]
pub fn paint_ok(text: impl Display, color: bool) -> String {
    paint(text, Style::new().green().bold(), color)
}

#[must_use]
pub fn paint_fail(text: impl Display, color: bool) -> String {
    paint(text, Style::new().red().bold(), color)
}

fn paint(text: impl Display, style: Style, color: bool) -> String {
    let s = text.to_string();
    if color {
        format!("{}", s.paint(style))
    } else {
        s
    }
}

/// Prefer a path relative to `root` (or cwd) for shorter status lines.
#[must_use]
pub fn display_path(path: &Path) -> String {
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(rel) = path.strip_prefix(&cwd)
    {
        let rendered = rel.display().to_string();
        if !rendered.is_empty() {
            return rendered;
        }
        return ".".to_string();
    }
    path.display().to_string()
}

/// Human-readable duration for Finished lines (Cargo-ish).
#[must_use]
pub fn format_elapsed(duration: Duration) -> String {
    if duration.as_secs() >= 60 {
        let mins = duration.as_secs() / 60;
        let secs = duration.as_secs() % 60;
        format!("{mins}m {secs:02}s")
    } else if duration.as_secs() > 0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else if duration.as_millis() > 0 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{}µs", duration.as_micros())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn status_line_is_aligned() {
        let line = format_status(Status::Finished, "transpile [debug] in 1.20s", false);
        assert!(line.starts_with("    Finished "));
        assert!(line.contains("transpile [debug]"));
    }

    #[test]
    fn error_status_is_lowercase_like_cargo() {
        let line = format_status(Status::Error, "failed to read config", false);
        assert!(line.starts_with("       error "));
    }
}
