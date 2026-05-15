use crate::{
    core::{DownloadConfig, Stream, mux::Streams, sub, vid},
    playlist::{MediaPlaylist, MediaType},
    progress::Progress,
};
use anyhow::Result;
use colored::Colorize;
use log::{info, warn};
use std::sync::atomic::AtomicBool;

pub async fn download_streams(
    config: &DownloadConfig,
    running: &AtomicBool,
    streams: Vec<MediaPlaylist>,
) -> Result<Streams> {
    let mut temp_files = Streams(Vec::new());
    let total = streams.len();

    for (i, stream) in streams.iter().enumerate() {
        info!(
            "DownLD [{}] {}",
            stream.media_type.to_string().green(),
            stream.display().cyan(),
        );

        if stream.segments.is_empty() {
            warn!("Stream skipped because no segments were found.");
            return Ok(temp_files);
        }

        let label = format!("{}/{}", i + 1, total);

        if stream.media_type == MediaType::Subtitles {
            temp_files.0.push(
                sub::download(
                    config,
                    running,
                    Progress::new(&label, stream.segments.len()),
                    stream,
                )
                .await?,
            );
        } else {
            temp_files.0.push(
                vid::download(
                    config,
                    running,
                    Progress::new(&label, stream.segments.len()),
                    stream,
                )
                .await?,
            );
        }
    }

    Ok(temp_files)
}

pub async fn download_stream(
    config: &DownloadConfig,
    running: &AtomicBool,
    label: &str,
    stream: &MediaPlaylist,
) -> Result<Option<Stream>> {
    info!(
        "DownLD [{}] {}",
        stream.media_type.to_string().green(),
        stream.display().cyan(),
    );

    if stream.segments.is_empty() {
        warn!("Stream skipped because no segments were found.");
        return Ok(None);
    }

    if stream.media_type == MediaType::Subtitles {
        return Ok(Some(
            sub::download(
                config,
                running,
                Progress::new(label, stream.segments.len()),
                stream,
            )
            .await?,
        ));
    } else {
        return Ok(Some(
            vid::download(
                config,
                running,
                Progress::new(label, stream.segments.len()),
                stream,
            )
            .await?,
        ));
    }
}
