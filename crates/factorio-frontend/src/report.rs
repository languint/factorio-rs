use std::io::{self, Write};
use std::ops::Range;
use std::path::Path;

use ariadne::{CharSet, Color, Config, IndexType, Label, LabelAttach, Report, ReportKind, Source};
use factorio_ir::lint::{Diagnostic, LintLevel};
use yansi::{Paint, Painted};

use crate::error::FrontendError;

type FileSpan = (String, Range<usize>);

fn file_span(filename: &str, range: Range<usize>) -> FileSpan {
    (filename.to_string(), range)
}

fn color_enabled() -> bool {
    io::IsTerminal::is_terminal(&io::stderr())
}

fn report_config() -> Config {
    Config::default()
        .with_color(color_enabled())
        .with_index_type(IndexType::Byte)
        .with_compact(false)
        .with_char_set(CharSet::Ascii)
        .with_label_attach(LabelAttach::Start)
}

fn paint_header(text: &str, color: Color) -> Painted<&str> {
    if color_enabled() {
        text.bold().fg(color)
    } else {
        Paint::new(text)
    }
}

fn paint_message(text: &str) -> String {
    if color_enabled() {
        text.bold().to_string()
    } else {
        text.to_string()
    }
}

fn write_cargo_header(
    mut writer: impl Write,
    kind: &str,
    color: Color,
    code: Option<&str>,
    message: &str,
) -> io::Result<()> {
    let tag = code.map_or_else(|| kind.to_string(), |code| format!("{kind}[{code}]"));
    writeln!(
        writer,
        "{}: {}",
        paint_header(&tag, color),
        paint_message(message)
    )
}

struct SkipFirstLine<W> {
    inner: W,
    seen_newline: bool,
    pending: Vec<u8>,
}

impl<W: Write> SkipFirstLine<W> {
    const fn new(inner: W) -> Self {
        Self {
            inner,
            seen_newline: false,
            pending: Vec::new(),
        }
    }
}

impl<W: Write> Write for SkipFirstLine<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.seen_newline {
            return self.inner.write(buf);
        }

        self.pending.extend_from_slice(buf);
        if let Some(idx) = self.pending.iter().position(|&b| b == b'\n') {
            self.seen_newline = true;
            let rest = self.pending.split_off(idx + 1);
            Vec::clear(&mut self.pending);
            if !rest.is_empty() {
                self.inner.write_all(&rest)?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.seen_newline && !self.pending.is_empty() {
            Vec::clear(&mut self.pending);
        }
        self.inner.flush()
    }
}

/// Write a transpile lint diagnostic in rustc/Cargo style.
///
/// # Errors
/// Returns when writing to `writer` fails.
pub fn write_diagnostic(
    mut writer: impl Write,
    filename: &str,
    source: &str,
    diagnostic: &Diagnostic,
) -> io::Result<()> {
    let (kind, color) = match diagnostic.level {
        LintLevel::Allow => return Ok(()),
        LintLevel::Warn => ("warning", Color::Yellow),
        LintLevel::Deny => ("error", Color::Red),
    };
    let report_kind = match diagnostic.level {
        LintLevel::Allow => return Ok(()),
        LintLevel::Warn => ReportKind::Warning,
        LintLevel::Deny => ReportKind::Error,
    };

    write_cargo_header(
        &mut writer,
        kind,
        color,
        Some(diagnostic.id.code()),
        &diagnostic.message,
    )?;

    let span = file_span(filename, diagnostic.loc.span.range());
    let mut builder = Report::build(report_kind, span.clone())
        .with_config(report_config())
        .with_message("")
        .with_label(
            Label::new(span)
                .with_message(diagnostic.message.clone())
                .with_color(color),
        );

    if let Some(note) = &diagnostic.loc.note {
        builder = builder.with_note(note.clone());
    }

    builder.finish().write(
        (filename.to_string(), Source::from(source)),
        SkipFirstLine::new(&mut writer),
    )?;
    write_cargo_footer(&mut writer, "help", diagnostic.id.help(), color)
}

fn write_cargo_footer(
    mut writer: impl Write,
    kind: &str,
    message: &str,
    color: Color,
) -> io::Result<()> {
    let label = format!("= {kind}:");
    if color_enabled() {
        writeln!(writer, "   {} {}", Paint::new(&label).fg(color), message)
    } else {
        writeln!(writer, "   {label} {message}")
    }
}

/// Write a frontend error in rustc/Cargo style.
///
/// # Errors
/// Returns when writing to `writer` fails.
pub fn write_frontend_error(
    mut writer: impl Write,
    filename: &str,
    source: &str,
    error: &FrontendError,
) -> io::Result<()> {
    if let FrontendError::Lint(diagnostic) = error {
        return write_diagnostic(writer, filename, source, diagnostic);
    }

    let message = error.report_message();
    write_cargo_header(&mut writer, "error", Color::Red, error.code(), &message)?;

    let span_range = error.location().map_or(0..0, |loc| loc.span.range());
    let span = file_span(filename, span_range);
    let mut builder = Report::build(ReportKind::Error, span.clone())
        .with_config(report_config())
        .with_message("");

    if error.location().is_some() {
        let label_message = error
            .location()
            .and_then(|loc| loc.note.clone())
            .unwrap_or_else(|| message.clone());
        builder = builder.with_label(
            Label::new(span)
                .with_message(label_message)
                .with_color(Color::Red),
        );
    }

    builder.finish().write(
        (filename.to_string(), Source::from(source)),
        SkipFirstLine::new(&mut writer),
    )?;
    if let Some(help) = error.help() {
        write_cargo_footer(&mut writer, "help", help, Color::Red)?;
    }
    Ok(())
}

/// Print a transpile lint diagnostic to stderr.
///
/// # Errors
/// Returns when writing to stderr fails.
pub fn eprint_diagnostic(filename: &str, source: &str, diagnostic: &Diagnostic) -> io::Result<()> {
    write_diagnostic(io::stderr(), filename, source, diagnostic)
}

/// Print a frontend error to stderr.
///
/// # Errors
/// Returns when writing to stderr fails.
pub fn eprint_frontend_error(
    filename: &str,
    source: &str,
    error: &FrontendError,
) -> io::Result<()> {
    write_frontend_error(io::stderr(), filename, source, error)
}

#[must_use]
pub fn display_filename(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorio_ir::lint::{LintId, LintLevel};
    use factorio_ir::span::{SourceLoc, SourceSpan};

    #[test]
    fn formats_unwrap_lint_cargo_style() {
        let source = "fn f(x: Option<i32>) -> i32 {\n    x.unwrap()\n}\n";
        let start = source.find("x.unwrap()").expect("snippet");
        let diagnostic = Diagnostic::new(
            LintId::Unwrap,
            LintLevel::Deny,
            "`.unwrap()` does not check for nil in Lua; use `if let Some(...)` instead",
            SourceLoc::new(SourceSpan::new(start, start + "x.unwrap()".len())),
        );
        let mut buf = Vec::new();
        write_diagnostic(&mut buf, "control.rs", source, &diagnostic).expect("write");
        let plain = strip_ansi(&String::from_utf8(buf).expect("utf8"));
        assert!(
            plain.starts_with("error[E0001]:"),
            "expected cargo-style EXXX header, got:\n{plain}"
        );
        assert!(plain.contains("x.unwrap()"), "missing snippet:\n{plain}");
        assert!(
            plain.contains("= help:"),
            "missing cargo-style help:\n{plain}"
        );
        assert!(
            plain.contains('^'),
            "expected caret underlines, got:\n{plain}"
        );
        assert!(
            !plain.contains('╰') && !plain.contains('─'),
            "expected ascii carets, not unicode box drawing:\n{plain}"
        );
    }

    #[test]
    fn formats_hard_frontend_error_with_code_and_help() {
        let source = "fn f() {\n    async { 1 }\n}\n";
        let start = source.find("async").expect("snippet");
        let error = FrontendError::UnsupportedExpression {
            location: SourceLoc::new(SourceSpan::new(start, start + "async".len()))
                .with_note("Async"),
        };
        let mut buf = Vec::new();
        write_frontend_error(&mut buf, "control.rs", source, &error).expect("write");
        let plain = strip_ansi(&String::from_utf8(buf).expect("utf8"));
        assert!(
            plain.starts_with("error[E0103]:"),
            "expected hard-error EXXX header, got:\n{plain}"
        );
        assert!(plain.contains("= help:"), "missing help footer:\n{plain}");
        assert!(plain.contains("async"), "missing snippet:\n{plain}");
    }

    #[test]
    fn formats_syn_error_with_span() {
        let source = "fn f( { }\n";
        let Err(error) = syn::parse_file(source) else {
            panic!("expected parse failure");
        };
        let frontend = FrontendError::from(error);
        assert!(frontend.location().is_some());
        let mut buf = Vec::new();
        write_frontend_error(&mut buf, "control.rs", source, &frontend).expect("write");
        let plain = strip_ansi(&String::from_utf8(buf).expect("utf8"));
        assert!(
            plain.starts_with("error[E0100]:"),
            "expected syn EXXX header, got:\n{plain}"
        );
        assert!(plain.contains("= help:"), "missing help footer:\n{plain}");
    }

    #[test]
    fn formats_warning_cargo_style() {
        let source = "fn f() { let _ = items[i]; }\n";
        let start = source.find("items[i]").expect("snippet");
        let diagnostic = Diagnostic::new(
            LintId::VariableIndex,
            LintLevel::Warn,
            "non-literal index is not shifted for Lua",
            SourceLoc::new(SourceSpan::new(start, start + "items[i]".len())),
        );
        let mut buf = Vec::new();
        write_diagnostic(&mut buf, "control.rs", source, &diagnostic).expect("write");
        let plain = strip_ansi(&String::from_utf8(buf).expect("utf8"));
        assert!(
            plain.starts_with("warning[E0004]:"),
            "expected warning EXXX header, got:\n{plain}"
        );
    }

    #[test]
    #[allow(clippy::literal_string_with_formatting_args, clippy::expect_used)]
    fn formats_format_spec_with_carets_on_placeholder() {
        let placeholder = "{".to_owned() + "y:.2}";
        let source = format!("fn f(n: f64) {{\n    println!(\"at {placeholder}\");\n}}\n");
        let start = source.find(&placeholder).expect("placeholder");
        let diagnostic = Diagnostic::new(
            LintId::FormatSpec,
            LintLevel::Deny,
            "format spec `:.2` is ignored when lowering (only `:?` / `:#?` are supported)",
            SourceLoc::new(SourceSpan::new(start, start + placeholder.len())),
        );
        let mut buf = Vec::new();
        write_diagnostic(&mut buf, "control.rs", &source, &diagnostic).expect("write");
        let plain = strip_ansi(&String::from_utf8(buf).expect("utf8"));
        assert!(
            plain.contains(&placeholder),
            "missing placeholder snippet:\n{plain}"
        );

        let underline = plain
            .lines()
            .find(|l| l.contains('^'))
            .expect("caret underline line");
        let mark_len = underline.chars().filter(|c| matches!(c, '^' | '|')).count();
        assert!(
            mark_len >= placeholder.len(),
            "underline should cover the placeholder, got:\n{plain}"
        );
    }

    fn strip_ansi(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' {
                if chars.next_if_eq(&'[').is_some() {
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                continue;
            }
            out.push(ch);
        }
        out
    }
}
