use crate::progress::ProgressCallback;
use crate::{
    error::{Error, Result},
    progress::Progress,
};
use log::{debug, info, trace};
use reqwest::{Client, StatusCode, Url, header};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use tokio::{
    fs::{self, File, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc,
    task::JoinSet,
};

pub const CHUNK_SIZE: u64 = 1024 * 1024 * 5; // 5 MiB

pub struct FileDownloader {
    chunk_size: u64,
    client: Client,
    progress: Option<Arc<dyn ProgressCallback>>,
    resume: bool,
    retries: u8,
    threads: u8,
}

impl FileDownloader {
    pub fn new(client: &Client) -> Self {
        Self {
            chunk_size: CHUNK_SIZE,
            client: client.clone(),
            progress: None,
            resume: true,
            retries: 10,
            threads: 5,
        }
    }

    pub fn chunk_size(mut self, chunk_size: u64) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    pub fn progress(mut self, progress: Arc<dyn ProgressCallback>) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn retries(mut self, retries: u8) -> Self {
        self.retries = retries;
        self
    }

    pub fn threads(mut self, threads: u8) -> Self {
        self.threads = threads.clamp(1, 16);
        self
    }

    pub fn resume(mut self, resume: bool) -> Self {
        self.resume = resume;
        self
    }

    pub async fn download(self, url: &str, output: impl AsRef<Path>) -> Result<()> {
        let url = url.parse::<Url>()?;
        let output = output.as_ref();

        debug!("Probing {} (HEAD)", url);
        let response = self.client.head(url.clone()).send().await?;
        let status = response.status();

        if !status.is_success() {
            return Err(Error::RequestFailed {
                url: url.to_string(),
                status,
                body: "HEAD request.".to_owned(),
            });
        }

        let content_length = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let accepts_ranges = response
            .headers()
            .get(header::ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v != "none");

        if content_length == 0 {
            bail!("Server returned content-length 0 for {}.", url);
        }

        let bytes_written = if self.resume {
            resume_offset(output).await
        } else {
            if output.exists() {
                fs::remove_file(output).await?;
            }
            0
        };

        if bytes_written >= content_length {
            info!("File already fully downloaded: {}", output.display());
            return Ok(());
        }

        if !accepts_ranges {
            debug!("Server does not support range requests, falling back to single stream.");
            return self
                .download_streaming(&url, output, content_length, bytes_written)
                .await;
        }

        info!(
            "Downloading {} ({} bytes, {} already written)",
            output.display(),
            content_length,
            bytes_written,
        );

        let chunks = compute_chunks(bytes_written, content_length, self.chunk_size);
        let total_chunks = chunks.len();

        let progress = Progress::new("dl", total_chunks, self.progress);
        let progress_handle = progress.spawn();

        let (tx, rx) = mpsc::channel::<(usize, Vec<u8>)>(self.threads as usize * 2);

        let writer_output = output.to_path_buf();
        let writer_progress = progress.clone();
        let writer_handle = tokio::spawn(async move {
            sequential_writer(
                rx,
                &writer_output,
                bytes_written,
                total_chunks,
                writer_progress,
            )
            .await
        });

        let mut set: JoinSet<Result<()>> = JoinSet::new();

        for (idx, (start, end)) in chunks.into_iter().enumerate() {
            let tx = tx.clone();
            let client = self.client.clone();
            let url = url.clone();
            let retries = self.retries;

            while set.len() >= self.threads as usize {
                if let Some(Ok(result)) = set.join_next().await {
                    result?;
                }
            }

            set.spawn(async move {
                let bytes = download_chunk(&client, &url, start, end, retries).await?;
                tx.send((idx, bytes))
                    .await
                    .map_err(|_| Error::Other("Writer channel closed.".into()))?;
                Ok(())
            });
        }

        // Wait for all download tasks to complete.
        while let Some(Ok(result)) = set.join_next().await {
            result?;
        }

        // Drop sender so the writer knows no more chunks are coming.
        drop(tx);

        // Wait for writer to finish flushing.
        writer_handle.await??;

        progress_handle.abort();
        progress.finish();

        Ok(())
    }

    async fn download_streaming(
        &self,
        url: &Url,
        output: &Path,
        content_length: u64,
        bytes_written: u64,
    ) -> Result<()> {
        let progress = Progress::new("dl", 1, self.progress.clone());
        let progress_handle = progress.spawn();

        let mut request = self.client.get(url.clone());

        // If resuming, request from the offset even in streaming mode.
        if bytes_written > 0 {
            request = request.header(header::RANGE, format!("bytes={}-", bytes_written));
        }

        let mut response = request.send().await?;

        if !response.status().is_success() {
            return Err(Error::RequestFailed {
                url: url.to_string(),
                status: response.status(),
                body: response.text().await?,
            });
        }

        let mut file = if bytes_written > 0 {
            OpenOptions::new().append(true).open(output).await?
        } else {
            File::create(output).await?
        };

        while let Some(bytes) = response.chunk().await? {
            file.write_all(&bytes).await?;
        }

        file.flush().await?;
        progress.update(content_length as usize);

        progress_handle.abort();
        progress.finish();

        Ok(())
    }
}

/// Check how many bytes have already been written for resume.
async fn resume_offset(path: &Path) -> u64 {
    match fs::metadata(path).await {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

/// Compute the byte ranges for each chunk.
///
/// Returns a vec of `(start, end)` inclusive byte ranges.
fn compute_chunks(offset: u64, total: u64, chunk_size: u64) -> Vec<(u64, u64)> {
    let mut chunks = Vec::new();
    let mut start = offset;

    while start < total {
        let end = (start + chunk_size - 1).min(total - 1);
        chunks.push((start, end));
        start = end + 1;
    }

    chunks
}

/// Download a single chunk with retry logic.
async fn download_chunk(
    client: &Client,
    url: &Url,
    start: u64,
    end: u64,
    max_retries: u8,
) -> Result<Vec<u8>> {
    let range = format!("bytes={}-{}", start, end);
    let mut avl_tries = max_retries;

    loop {
        trace!("Downloading range {} from {}", range, url);

        match client
            .get(url.clone())
            .header(header::RANGE, &range)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();

                if status.is_success() || status == StatusCode::PARTIAL_CONTENT {
                    return Ok(response.bytes().await?.to_vec());
                }

                if avl_tries == 0 {
                    return Err(Error::RequestFailed {
                        url: url.to_string(),
                        status,
                        body: response.text().await?,
                    });
                }
            }
            Err(e) => {
                if avl_tries == 0 {
                    return Err(Error::RequestFailed {
                        url: url.to_string(),
                        status: e.status().unwrap_or_default(),
                        body: format!("GET range {}", range),
                    });
                }
            }
        }

        trace!("Retrying range {} ({})", range, avl_tries);
        avl_tries -= 1;
    }
}

/// Receives chunks from the channel and writes them to the output file in order.
///
/// Out-of-order chunks are buffered in a `BTreeMap` until the preceding chunk
/// arrives, then all consecutive buffered chunks are flushed.
async fn sequential_writer(
    mut rx: mpsc::Receiver<(usize, Vec<u8>)>,
    output: &Path,
    bytes_written: u64,
    total_chunks: usize,
    progress: Progress,
) -> Result<()> {
    let mut file = if bytes_written > 0 {
        OpenOptions::new().append(true).open(output).await?
    } else {
        File::create(output).await?
    };

    let mut next_idx = 0usize;
    let mut pending: BTreeMap<usize, Vec<u8>> = BTreeMap::new();

    while let Some((idx, data)) = rx.recv().await {
        if idx == next_idx {
            // This is the next expected chunk — write directly.
            let size = data.len();
            file.write_all(&data).await?;
            progress.update(size);
            next_idx += 1;

            // Flush any consecutive buffered chunks.
            while let Some(buffered) = pending.remove(&next_idx) {
                let size = buffered.len();
                file.write_all(&buffered).await?;
                progress.update(size);
                next_idx += 1;
            }

            file.flush().await?;
        } else {
            // Out of order — buffer it.
            pending.insert(idx, data);
        }
    }

    if next_idx < total_chunks {
        bail!(
            "Download incomplete: received {}/{} chunks.",
            next_idx,
            total_chunks,
        );
    }

    file.flush().await?;
    Ok(())
}
