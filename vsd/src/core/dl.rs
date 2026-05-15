use crate::{
    core::{STREAM_DL_IDX, Stream, mux::Streams, sub, vid},
    playlist::{MediaPlaylist, MediaType},
    progress::Progress,
    utils::Query,
};
use anyhow::Result;
use colored::Colorize;
use log::{info, warn};
use reqwest::Client;
use std::{collections::HashMap, path::PathBuf, sync::atomic::Ordering};

pub async fn download_streams(
    client: &Client,
    query: &Query,
    directory: Option<&PathBuf>,
    keys: &HashMap<String, String>,
    streams: Vec<MediaPlaylist>,
) -> Result<Streams> {
    let mut temp_files = Streams(Vec::new());
    let total = streams.len();

    for stream in streams {
        info!(
            "DownLD [{}] {}",
            stream.media_type.to_string().green(),
            stream.display().cyan(),
        );

        if stream.segments.is_empty() {
            warn!("Stream skipped because no segments were found.");
            return Ok(temp_files);
        }

        if stream.media_type == MediaType::Subtitles {
            temp_files.0.push(
                sub::download(
                    client,
                    query,
                    directory,
                    Progress::new(
                        &format!("{}/{}", STREAM_DL_IDX.fetch_add(1, Ordering::SeqCst), total),
                        stream.segments.len(),
                    ),
                    &stream,
                )
                .await?,
            );
        } else {
            temp_files.0.push(
                vid::download(
                    client,
                    query,
                    directory,
                    keys,
                    Progress::new(
                        &format!("{}/{}", STREAM_DL_IDX.fetch_add(1, Ordering::SeqCst), total),
                        stream.segments.len(),
                    ),
                    &stream,
                )
                .await?,
            );
        }
    }

    Ok(temp_files)
}

pub async fn download_stream(
    client: &Client,
    query: &Query,
    directory: Option<&PathBuf>,
    keys: &HashMap<String, String>,
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
                client,
                query,
                directory,
                Progress::new(
                    &STREAM_DL_IDX.fetch_add(1, Ordering::SeqCst).to_string(),
                    stream.segments.len(),
                ),
                &stream,
            )
            .await?,
        ));
    } else {
        return Ok(Some(
            vid::download(
                client,
                query,
                directory,
                keys,
                Progress::new(
                    &STREAM_DL_IDX.fetch_add(1, Ordering::SeqCst).to_string(),
                    stream.segments.len(),
                ),
                &stream,
            )
            .await?,
        ));
    }
}
