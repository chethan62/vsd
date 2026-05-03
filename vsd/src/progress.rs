use colored::Colorize;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::task::JoinHandle;

struct ProgressInner {
    counter: usize,
    id: String,
    session_counter: usize,
    session_bytes: usize,
    total: usize,
    timer: Instant,
    total_bytes: usize,
}

#[derive(Clone)]
pub struct Progress {
    inner: Arc<Mutex<ProgressInner>>,
}

impl Progress {
    pub fn new(id: &str, total: usize) -> Self {
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

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let done = {
                    let inner = inner.lock().unwrap();
                    Self::render(&inner);
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
        Self::render(&inner);
        eprintln!();
    }

    fn render(inner: &ProgressInner) {
        if inner.counter == 0 {
            return;
        }

        let remaining_bytes =
            ((inner.total_bytes as f64 / inner.counter as f64) * inner.total as f64) as usize;

        let percent = if inner.total > 0 {
            (inner.counter as f64 / inner.total as f64 * 100.0) as usize
        } else {
            100
        };

        let elapsed_secs = inner.timer.elapsed().as_secs_f64();

        let speed = if inner.session_counter > 0 {
            inner.session_bytes as f64 / elapsed_secs
        } else {
            0.0
        };

        let rate = if inner.session_counter > 0 {
            inner.session_counter as f64 / elapsed_secs
        } else {
            0.0
        };

        let eta_secs = if rate > 0.0 {
            (inner.total.saturating_sub(inner.counter) as f64 / rate) as usize
        } else {
            0
        };

        let stderr = io::stderr();
        let mut handle = stderr.lock();
        write!(
            handle,
            "\r\x1B[2K{}#({}) {}/~{}{} PT:{} DL:{} ETA:{}{}",
            "[".magenta(),
            inner.id,
            ByteSize(inner.total_bytes),
            ByteSize(remaining_bytes),
            format!("({}%)", percent).cyan(),
            format!("{}/{}", inner.counter, inner.total).cyan(),
            ByteSize(speed as usize).to_string().green(),
            Eta(eta_secs).to_string().yellow(),
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
