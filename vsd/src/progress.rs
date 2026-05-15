use colored::Colorize;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::task::JoinHandle;

/// Snapshot of download progress for a single stream.
pub struct ProgressState {
    /// Stream label (e.g. "1/3")
    pub label: String,
    /// Number of parts downloaded so far
    pub downloaded_parts: usize,
    /// Total number of parts
    pub total_parts: usize,
    /// Total bytes downloaded so far
    pub downloaded_bytes: usize,
    /// Estimated total bytes (extrapolated from current progress)
    pub estimated_bytes: usize,
    /// Download speed in bytes per second
    pub speed_bps: f64,
    /// Estimated time remaining in seconds
    pub eta_seconds: usize,
    /// Completion percentage (0–100)
    pub percent: u8,
}

/// Trait for receiving download progress updates.
///
/// Implement this trait to receive progress updates in your own UI,
/// logging system, or any other consumer. When no callback is provided,
/// the built-in terminal progress bar is used instead.
///
/// # Example
///
/// ```rust,no_run
/// use vsd::progress::{ProgressCallback, ProgressState};
///
/// struct MyProgress;
///
/// impl ProgressCallback for MyProgress {
///     fn on_progress(&self, state: &ProgressState) {
///         println!("{}% ({}/{})", state.percent, state.downloaded_segments, state.total_segments);
///     }
///     fn on_finish(&self, state: &ProgressState) {
///         println!("Done! {} bytes", state.downloaded_bytes);
///     }
/// }
/// ```
pub trait ProgressCallback: Send + Sync {
    /// Called periodically (roughly once per second) with the current progress.
    fn on_progress(&self, state: &ProgressState);
    /// Called once when the stream download completes.
    fn on_finish(&self, state: &ProgressState);
}

struct ProgressInner {
    counter: usize,
    id: String,
    session_counter: usize,
    session_bytes: usize,
    total: usize,
    timer: Instant,
    total_bytes: usize,
}

impl ProgressInner {
    fn state(&self) -> ProgressState {
        let estimated_bytes = if self.counter > 0 {
            ((self.total_bytes as f64 / self.counter as f64) * self.total as f64) as usize
        } else {
            0
        };

        let percent = if self.total > 0 {
            (self.counter as f64 / self.total as f64 * 100.0) as u8
        } else {
            100
        };

        let elapsed_secs = self.timer.elapsed().as_secs_f64();

        let speed_bps = if self.session_counter > 0 {
            self.session_bytes as f64 / elapsed_secs
        } else {
            0.0
        };

        let rate = if self.session_counter > 0 {
            self.session_counter as f64 / elapsed_secs
        } else {
            0.0
        };

        let eta_seconds = if rate > 0.0 {
            (self.total.saturating_sub(self.counter) as f64 / rate) as usize
        } else {
            0
        };

        ProgressState {
            label: self.id.clone(),
            downloaded_parts: self.counter,
            total_parts: self.total,
            downloaded_bytes: self.total_bytes,
            estimated_bytes,
            speed_bps,
            eta_seconds,
            percent,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Progress {
    inner: Arc<Mutex<ProgressInner>>,
    callback: Option<Arc<dyn ProgressCallback>>,
}

impl Progress {
    pub fn new(id: &str, total: usize, callback: Option<Arc<dyn ProgressCallback>>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ProgressInner {
                counter: 0,
                id: id.to_owned(),
                session_counter: 0,
                session_bytes: 0,
                total,
                timer: Instant::now(),
                total_bytes: 0,
            })),
            callback,
        }
    }

    pub fn update(&self, chunk_bytes: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.counter += 1;
        inner.session_counter += 1;
        inner.total_bytes += chunk_bytes;
        inner.session_bytes += chunk_bytes;
    }

    pub fn skip(&self, file_bytes: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.counter += 1;
        inner.total_bytes += file_bytes;
    }

    pub fn spawn(&self) -> JoinHandle<()> {
        let inner = self.inner.clone();
        let callback = self.callback.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let done = {
                    let inner = inner.lock().unwrap();
                    if let Some(cb) = &callback {
                        cb.on_progress(&inner.state());
                    } else {
                        Self::render(&inner);
                    }
                    inner.counter >= inner.total
                };
                if done {
                    break;
                }
            }
        })
    }

    pub fn finish(&self) {
        let inner = self.inner.lock().unwrap();
        if let Some(cb) = &self.callback {
            cb.on_finish(&inner.state());
        } else {
            Self::render(&inner);
            eprintln!();
        }
    }

    fn render(inner: &ProgressInner) {
        if inner.counter == 0 {
            return;
        }

        let state = inner.state();

        let stderr = io::stderr();
        let mut handle = stderr.lock();
        write!(
            handle,
            "\r\x1B[2K{}#({}) {}/~{}{} PT:{} DL:{} ETA:{}{}",
            "[".magenta(),
            state.label,
            ByteSize(state.downloaded_bytes),
            ByteSize(state.estimated_bytes),
            format!("({}%)", state.percent).cyan(),
            format!("{}/{}", state.downloaded_parts, state.total_parts).cyan(),
            ByteSize(state.speed_bps as usize).to_string().green(),
            Eta(state.eta_seconds).to_string().yellow(),
            "]".magenta(),
        )
        .unwrap();
        handle.flush().unwrap();
    }
}

pub struct ByteSize(pub usize);

impl std::fmt::Display for ByteSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const KIB: f64 = 1024.0;
        const MIB: f64 = KIB * 1024.0;
        const GIB: f64 = MIB * 1024.0;

        let bytes = self.0 as f64;

        if bytes >= GIB {
            write!(f, "{:.1}GiB", bytes / GIB)
        } else if bytes >= MIB {
            write!(f, "{:.1}MiB", bytes / MIB)
        } else if bytes >= KIB {
            write!(f, "{:.1}KiB", bytes / KIB)
        } else {
            write!(f, "{}B", self.0)
        }
    }
}

struct Eta(usize);

impl std::fmt::Display for Eta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_seconds = self.0;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            write!(f, "{}h{}m{}s", hours, minutes, seconds)
        } else if minutes > 0 {
            write!(f, "{}m{}s", minutes, seconds)
        } else {
            write!(f, "{}s", seconds)
        }
    }
}
