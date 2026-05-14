use crate::{
    core::{
        MAX_THREADS, NO_RESUME, RUNNING, STREAM_DL_IDX,
        mux::{Stream, Streams},
    },
    playlist::{MediaPlaylist, MediaType},
    progress::Progress,
    utils::{self, QUERY},
};
use anyhow::{Result, bail};
use colored::Colorize;
use log::{debug, info, warn};
use reqwest::{Client, header};
use std::{path::PathBuf, sync::atomic::Ordering};
use tokio::{fs::File, io::AsyncWriteExt, task::JoinSet};
use vsd_mp4::text::{Mp4TtmlParser, Mp4VttParser, ttml_text_parser};

enum SubtitleType {
    Mp4Vtt,
    Mp4Ttml,
    SrtText,
    TtmlText,
    Unknown,
    VttText,
}

fn detect_codec(codecs: Option<&str>, data: &[u8], ext: &str) -> (&'static str, SubtitleType) {
    if let Some(codecs) = codecs {
        match codecs.to_lowercase().as_str() {
            "vtt" => return ("vtt", SubtitleType::VttText),
            "wvtt" => return ("vtt", SubtitleType::Mp4Vtt),
            "stpp" | "stpp.ttml" | "stpp.ttml.im1t" => return ("srt", SubtitleType::Mp4Ttml),
            _ => (),
        }
    }

    if data.starts_with(b"WEBVTT") || ext == "vtt" {
        ("vtt", SubtitleType::VttText)
    } else if data.starts_with(b"1") || ext == "srt" {
        ("srt", SubtitleType::SrtText)
    } else if data.starts_with(b"<?xml") || data.starts_with(b"<tt") || ext == "ttml" {
        ("srt", SubtitleType::TtmlText)
    } else if Mp4VttParser::from_init(data).is_ok() {
        ("vtt", SubtitleType::Mp4Vtt)
    } else if Mp4TtmlParser::from_init(data).is_ok() {
        ("srt", SubtitleType::Mp4Ttml)
    } else {
        warn!("Stream uses unknown subtitle codec.");
        ("txt", SubtitleType::Unknown)
    }
}

pub async fn download_subtitle_streams(
    client: &Client,
    streams: &[MediaPlaylist],
    query: &QUERY,
    directory: Option<&PathBuf>,
    temp_files: &mut Streams,
) -> Result<()> {
    let total = streams.len();

    for stream in streams {
        if stream.media_type != MediaType::Subtitles {
            continue;
        }

        download_subtitle_stream(
            client,
            stream,
            query,
            directory,
            temp_files,
            Progress::new(
                &format!("{}/{}", STREAM_DL_IDX.fetch_add(1, Ordering::SeqCst), total),
                stream.segments.len(),
            ),
        )
        .await?;
    }

    Ok(())
}

async fn download_subtitle_stream(
    client: &Client,
    stream: &MediaPlaylist,
    query: &QUERY,
    directory: Option<&PathBuf>,
    temp_files: &mut Streams,
    pb: Progress,
) -> Result<()> {
    info!(
        "DownLD [{}] {}",
        stream.media_type.to_string().green(),
        stream.display().cyan(),
    );

    if stream.segments.is_empty() {
        warn!("Stream skipped because no segments were found.");
        return Ok(());
    }

    let base_url = stream.uri.parse()?;
    let ext = stream.extension();
    let mut data = Vec::new();
    let mut temp_file = stream.path(directory);

    if let Some(mut bytes) = stream.fetch_init(client, &base_url, query).await? {
        data.append(&mut bytes);
    }

    let segment = &stream.segments[0];
    let url = base_url.join(&segment.uri)?;
    let mut request = client.get(url).query(query);

    if let Some(range) = &segment.range {
        request = request.header(header::RANGE, range);
    }

    let response = request.send().await?;
    let mut bytes = utils::fetch_bytes(response).await?;
    let size = bytes.len();
    data.append(&mut bytes);

    let (ext, codec) = detect_codec(stream.codecs.as_deref(), &data, ext);

    temp_file = temp_file.with_extension(ext);
    temp_files.0.push(Stream {
        language: stream.language.clone(),
        media_type: stream.media_type.clone(),
        path: temp_file.clone(),
    });

    if temp_file.exists() && !NO_RESUME.load(Ordering::SeqCst) {
        info!(
            "Saving [{}] {} (downloaded)",
            stream.media_type.to_string().green(),
            temp_file.to_string_lossy()
        );
        return Ok(());
    } else {
        info!(
            "Saving [{}] {}",
            stream.media_type.to_string().green(),
            temp_file.to_string_lossy()
        );
    }

    pb.update(size);

    let remaining = &stream.segments[1..];

    if !remaining.is_empty() {
        let pb_handle = pb.spawn();
        let max_threads = MAX_THREADS.load(Ordering::SeqCst) as usize;
        let mut set: JoinSet<Result<(usize, Vec<u8>)>> = JoinSet::new();
        let mut results = vec![None; remaining.len()];

        for (i, segment) in remaining.iter().enumerate() {
            if !RUNNING.load(Ordering::SeqCst) {
                break;
            }

            while set.len() >= max_threads {
                if let Some(Ok(result)) = set.join_next().await {
                    let (i, bytes) = match result {
                        Ok(v) => v,
                        Err(e) => {
                            set.abort_all();
                            bail!(e);
                        }
                    };
                    pb.update(bytes.len());
                    results[i] = Some(bytes);
                }
            }

            let url = base_url.join(&segment.uri)?;
            let mut request = client.get(url).query(query);

            if let Some(range) = &segment.range {
                request = request.header(header::RANGE, range);
            }

            set.spawn(async move {
                let response = request.send().await?;
                let bytes = utils::fetch_bytes(response).await?;
                Ok((i, bytes))
            });
        }

        while let Some(Ok(result)) = set.join_next().await {
            let (i, bytes) = match result {
                Ok(v) => v,
                Err(e) => {
                    set.abort_all();
                    bail!(e);
                }
            };
            pb.update(bytes.len());
            results[i] = Some(bytes);
        }

        for mut bytes in results.into_iter().flatten() {
            data.append(&mut bytes);
        }

        pb_handle.abort();
    }

    pb.finish();

    if !RUNNING.load(Ordering::SeqCst) {
        warn!("Download interrupted.");
        std::process::exit(0);
    }

    let output = match codec {
        SubtitleType::Mp4Vtt => {
            debug!("Extracting wvtt subtitles.");
            let vtt = Mp4VttParser::from_init(&data)?;
            vtt.parse(&data, None)?.as_vtt().into_bytes()
        }
        SubtitleType::Mp4Ttml => {
            debug!("Extracting stpp subtitles.");
            let ttml = Mp4TtmlParser::from_init(&data)?;
            ttml.parse(&data)?.as_srt().into_bytes()
        }
        SubtitleType::TtmlText => {
            debug!("Extracting ttml+xml subtitles.");
            ttml_text_parser::parse_bytes(&data)?
                .into_subtitles()
                .as_srt()
                .into_bytes()
        }
        _ => data,
    };

    File::create(&temp_file).await?.write_all(&output).await?;

    Ok(())
}
