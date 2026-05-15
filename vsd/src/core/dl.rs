use crate::{
    core::{DownloadConfig, Stream, mux::Muxer, sub, vid},
    playlist::{MediaPlaylist, MediaType},
    progress::{Progress, ProgressCallback},
};
use anyhow::Result;
use colored::Colorize;
use log::{info, warn};
use std::sync::{Arc, atomic::AtomicBool};

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
            return Ok(muxer);
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

pub async fn download_stream(
    config: &DownloadConfig,
    running: &AtomicBool,
    callback: Arc<dyn ProgressCallback>,
    stream: &MediaPlaylist,
) -> Result<Option<Stream>> {
    if stream.segments.is_empty() {
        return Ok(None);
    }

    let pb = Progress::new(&stream.id, stream.segments.len(), Some(callback));

    if stream.media_type == MediaType::Subtitles {
        Ok(Some(sub::download(config, running, pb, stream).await?))
    } else {
        Ok(Some(vid::download(config, running, pb, stream).await?))
    }
}
