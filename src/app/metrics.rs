use std::time::{Duration, Instant};

/// Collects timing data for key operations when running in debug mode.
/// The main purpose is logging to the trace file (`/tmp/git-log-ratatui.log`),
/// a summary is also shown in the help bar.
#[derive(Clone, Debug)]
pub struct DebugMetrics {
    /// Timestamp of the last commit request (set by request_commits).
    pub commit_request_time: Option<Instant>,
    /// Timestamp of the last branch request (set by request_branches).
    pub branch_request_time: Option<Instant>,
    /// Timestamp of the last diff request (set by request_diff).
    pub diff_request_time: Option<Instant>,
    /// Duration of the last frame (work + render).
    pub last_frame: Option<Duration>,
    /// Duration of the last commit fetch (round-trip: request to result).
    pub last_commit_fetch: Option<Duration>,
    /// How many commits were fetched in the last worker call.
    pub last_commit_count: u64,
    /// Duration of the last branch fetch.
    pub last_branch_fetch: Option<Duration>,
    /// How many branch entries were fetched.
    pub last_branch_count: usize,
    /// Duration of the last diff fetch (round-trip: request to result).
    pub last_diff: Option<Duration>,
    /// Number of diff lines in the last diff.
    pub last_diff_lines: usize,
    /// Number of changed files in the last diff.
    pub last_diff_files: usize,
    /// Duration of the last enrich_commits call (set internally by git operations).
    pub last_enrich: Option<Duration>,
    /// Number of commits enriched.
    pub last_enrich_count: usize,
    /// Duration of the last build_all_lines call.
    pub last_diff_render: Option<Duration>,
    /// Duration of the last process_git_results poll.
    pub last_poll: Option<Duration>,
    /// Rolling average frame time (ms).
    pub avg_frame_ms: f64,
}

impl DebugMetrics {
    pub fn new() -> Self {
        DebugMetrics {
            commit_request_time: None,
            branch_request_time: None,
            diff_request_time: None,
            last_frame: None,
            last_commit_fetch: None,
            last_commit_count: 0,
            last_branch_fetch: None,
            last_branch_count: 0,
            last_diff: None,
            last_diff_lines: 0,
            last_diff_files: 0,
            last_enrich: None,
            last_enrich_count: 0,
            last_diff_render: None,
            last_poll: None,
            avg_frame_ms: 0.0,
        }
    }

    /// Update rolling average with a new frame duration.
    pub fn record_frame(&mut self, duration: Duration) {
        let ms = duration.as_millis() as f64;
        if self.avg_frame_ms == 0.0 {
            self.avg_frame_ms = ms;
        } else {
            self.avg_frame_ms = self.avg_frame_ms * 0.9 + ms * 0.1;
        }
        self.last_frame = Some(duration);
    }

    /// Format key metrics for display in the debug overlay.
    pub fn format_summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(d) = self.last_frame {
            parts.push(format!("frame={}ms", d.as_millis()));
        }
        parts.push(format!("avg={:.0}ms", self.avg_frame_ms));

        if let Some(d) = self.last_commit_fetch {
            parts.push(format!(
                "cmt={}ms({})",
                d.as_millis(),
                format_count(self.last_commit_count)
            ));
        }
        if let Some(d) = self.last_diff {
            parts.push(format!(
                "diff={}ms({}L/{}F)",
                d.as_millis(),
                format_count(self.last_diff_lines as u64),
                self.last_diff_files
            ));
        }
        if let Some(d) = self.last_branch_fetch {
            parts.push(format!(
                "branch={}ms({})",
                d.as_millis(),
                self.last_branch_count
            ));
        }
        if let Some(d) = self.last_enrich {
            parts.push(format!(
                "enr={}ms({})",
                d.as_millis(),
                format_count(self.last_enrich_count as u64)
            ));
        }
        if let Some(d) = self.last_diff_render {
            parts.push(format!("bld={}ms", d.as_millis()));
        }
        if let Some(d) = self.last_poll {
            parts.push(format!("poll={}µs", d.as_micros()));
        }

        parts.join(" ")
    }
}

impl Default for DebugMetrics {
    fn default() -> Self {
        Self::new()
    }
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        if n < 10_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            format!("{}k", n / 1_000)
        }
    } else {
        n.to_string()
    }
}
