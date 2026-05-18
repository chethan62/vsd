use crate::{
    core::{DownloadConfig, mux::Muxer, sub, vid},
    error::Result,
    playlist::{MediaPlaylist, MediaType},
    progress::Progress,
};
use colored::Colorize;
use log::{info, warn};
use std::sync::atomic::AtomicBool;

pub async fn download_streams(
    config: &DownloadConfig,
    running: &AtomicBool,
    streams: Vec<MediaPlaylist>,
) -> Result<Muxer> {
    let mut muxer = Muxer(Vec::new());
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
                .push(sub::download(config, running, pb, stream).await?);
        } else {
            muxer
                .0
                .push(vid::download(config, running, pb, stream).await?);
        }
    }

    Ok(muxer)
}
