use crate::{
    core::{DownloadConfig, mux::Muxer, sub, vid},
    error::Result,
    playlist::{MediaPlaylist, MediaType},
    progress::Progress,
};
use colored::Colorize;
use log::{info, warn};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub async fn download_streams(
    config: &DownloadConfig,
    streams: Vec<MediaPlaylist>,
) -> Result<Muxer> {
    let running = Arc::new(AtomicBool::new(true));

    let ctrlc = running.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() && ctrlc.load(Ordering::SeqCst) {
            warn!("Aborting download due to Ctrl+C.");
            ctrlc.store(false, Ordering::SeqCst);
        }

        if tokio::signal::ctrl_c().await.is_ok() {
            warn!("Force exiting due to Ctrl+C.");
            std::process::exit(1);
        }
    });

    let mut muxer = Muxer(Vec::new());
    let running = running.clone();
    let total = streams.len();

    for (i, stream) in streams.iter().enumerate() {
        info!(
            "DownLD [{}] {}",
            stream.media_type.to_string().green(),
            stream.display().cyan(),
        );

        if stream.segments.is_empty() {
            warn!("Stream skipped because no segments were found.");
            continue;
        }

        let label = format!("{}/{}", i + 1, total);
        let pb = Progress::new(&label, stream.segments.len(), None);

        if stream.media_type == MediaType::Subtitles {
            muxer
                .0
                .push(sub::download(config, &running, pb, stream).await?);
        } else {
            muxer
                .0
                .push(vid::download(config, &running, pb, stream).await?);
        }
    }

    Ok(muxer)
}
