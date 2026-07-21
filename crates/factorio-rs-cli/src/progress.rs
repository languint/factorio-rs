//! Progress UI for `factorio-rs build`.
//!
//! One progress line stays pinned at the bottom of the terminal; log lines are
//! printed above it via [`MultiProgress::println`].

use std::io::IsTerminal;
use std::path::Path;
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::status::{self, Status, format_elapsed};

/// Summary printed when a build succeeds.
#[derive(Debug, Clone)]
pub struct BuildFinish {
    pub profile: String,
    /// Project-relative output directory (e.g. `dist`).
    pub output_dir: String,
    pub file_count: usize,
}

pub struct BuildProgress {
    multi: MultiProgress,
    /// Single bottom-pinned line (spinner and/or file bar).
    bar: ProgressBar,
    phases: Vec<PhaseRecord>,
    current: Option<(String, Instant)>,
    enabled: bool,
    total_started: Instant,
    /// When true, the bar shows `{pos}/{len}` for source files / modules.
    files_active: bool,
}

struct PhaseRecord {
    name: String,
    duration: Duration,
}

impl BuildProgress {
    #[must_use]
    pub fn start() -> Self {
        // Pin to stdout so log lines and the bar share one stream (scroll above).
        let enabled = stdout_progress_enabled();
        let multi = MultiProgress::with_draw_target(if enabled {
            ProgressDrawTarget::stdout()
        } else {
            ProgressDrawTarget::hidden()
        });
        let bar = if enabled {
            let pb = multi.add(ProgressBar::new_spinner());
            pb.set_style(spinner_style());
            pb.enable_steady_tick(Duration::from_millis(80));
            pb.set_message("Starting...");
            pb
        } else {
            ProgressBar::hidden()
        };

        Self {
            multi,
            bar,
            phases: Vec::new(),
            current: None,
            enabled,
            total_started: Instant::now(),
            files_active: false,
        }
    }

    /// Print a log line above the pinned progress line.
    pub fn println(&self, msg: impl AsRef<str>) {
        if self.enabled {
            let _ = self.multi.println(msg.as_ref());
        } else {
            println!("{}", msg.as_ref());
        }
    }

    /// Begin a named phase (ends any previous phase / file mode).
    pub fn begin(&mut self, name: impl Into<String>) {
        self.clear_files_mode();
        self.finish_current();
        let name = name.into();
        if self.enabled {
            self.bar.set_style(spinner_style());
            self.bar
                .set_message(format!("{}...", short_phase_label(&name)));
        }
        self.current = Some((name, Instant::now()));
    }

    /// Switch the pinned line to a determinate bar over `total` items.
    pub fn start_files(&mut self, total: u64, label: &str) {
        self.clear_files_mode();
        if !self.enabled || total == 0 {
            return;
        }
        self.bar.set_style(file_bar_style());
        self.bar.set_length(total);
        self.bar.set_position(0);
        self.bar.set_message(short_phase_label(label).to_string());
        self.files_active = true;
    }

    /// Advance the file bar and show the current path / module name.
    pub fn tick_file(&self, display: impl AsRef<str>) {
        if !self.enabled {
            return;
        }
        if self.files_active {
            self.bar.set_message(truncate_msg(display.as_ref(), 32));
            self.bar.inc(1);
        } else {
            self.bar.set_message(display.as_ref().to_string());
        }
    }

    /// Run `f` while temporarily hiding progress (for diagnostics on stderr).
    pub fn suspend<R>(&self, f: impl FnOnce() -> R) -> R {
        if self.enabled {
            self.multi.suspend(f)
        } else {
            f()
        }
    }

    fn clear_files_mode(&mut self) {
        if self.files_active {
            self.files_active = false;
            if self.enabled {
                self.bar.set_style(spinner_style());
                // Keep spinning; length is irrelevant for spinner style.
                self.bar.unset_length();
            }
        }
    }

    fn finish_current(&mut self) {
        if let Some((name, started)) = self.current.take() {
            self.phases.push(PhaseRecord {
                name,
                duration: started.elapsed(),
            });
        }
    }

    /// Print a Finished summary (and optional phase timings), then clear the bar.
    pub fn finish(mut self, summary: &BuildFinish) {
        self.clear_files_mode();
        self.finish_current();

        let total = self.total_started.elapsed();
        let color = status::color_stdout();
        let files = if summary.file_count == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", summary.file_count)
        };
        let output = if summary.output_dir.ends_with('/') {
            summary.output_dir.clone()
        } else {
            format!("{}/", summary.output_dir)
        };
        let message = format!(
            "transpile [{}] -> {output} ({files}) in {}",
            summary.profile,
            format_elapsed(total)
        );
        self.println(status::format_status(Status::Finished, message, color));

        if self.phases.len() > 1 {
            let timing = self
                .phases
                .iter()
                .map(|phase| {
                    format!(
                        "{} {}",
                        short_phase_label(&phase.name),
                        format_elapsed(phase.duration)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            self.println(status::dim(
                format!("{:width$}{timing}", "", width = STATUS_DETAIL_INDENT),
                color,
            ));
        }

        if self.enabled {
            self.bar.finish_and_clear();
        }
    }

    /// Abandon progress without printing a success summary.
    pub fn abandon(mut self) {
        self.clear_files_mode();
        if self.enabled {
            self.bar.finish_and_clear();
        }
    }
}

const STATUS_DETAIL_INDENT: usize = 13; // aligns under status message column

fn stdout_progress_enabled() -> bool {
    // Progress bars need a real TTY; color-force env vars are not enough.
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn short_phase_label(name: &str) -> &str {
    match name {
        "Typecheck" => "Typechecking",
        "Prepare" => "Preparing",
        "Lower" => "Lowering",
        "Emit" => "Emitting",
        "Finalize" => "Finalizing",
        other => other,
    }
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
}

#[allow(clippy::literal_string_with_formatting_args)]
fn file_bar_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg:<32} [{bar:20.cyan/blue}] {pos}/{len}")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-")
}

/// Display path relative to `root` when possible.
#[must_use]
pub fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).map_or_else(
        |_| path.display().to_string(),
        |rel| rel.display().to_string(),
    )
}

fn truncate_msg(msg: &str, width: usize) -> String {
    if msg.chars().count() <= width {
        return msg.to_string();
    }
    let keep = width.saturating_sub(1);
    let truncated: String = msg.chars().take(keep).collect();
    format!("{truncated}...")
}
