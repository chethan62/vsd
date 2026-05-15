use crate::{
    core::{DownloadConfig, mux::Stream},
    error::{Error, Result},
    playlist::MediaPlaylist,
    progress::Progress,
    utils,
};
use colored::Colorize;
use log::{debug, info, warn};
use reqwest::{Url, header};
use std::sync::atomic::{AtomicBool, Ordering};
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

pub async fn download(
    config: &DownloadConfig,
    running: &AtomicBool,
    pb: Progress,
    stream: &MediaPlaylist,
) -> Result<Stream> {
    let base_url = stream.uri.parse::<Url>()?;
    let ext = stream.extension();
    let mut data = Vec::new();
    let mut temp_file = stream.path(config.directory.as_ref());

    if let Some(mut bytes) = stream.fetch_init(config).await? {
        data.append(&mut bytes);
    }

    let segment = &stream.segments[0];
    let url = base_url.join(&segment.uri)?;
    let mut request = config.client.get(url).query(&config.query);

    if let Some(range) = &segment.range {
        request = request.header(header::RANGE, range);
    }

    let response = request.send().await?;
    let mut bytes = utils::fetch_bytes(response).await?;
    let size = bytes.len();
    data.append(&mut bytes);

    let (ext, codec) = detect_codec(stream.codecs.as_deref(), &data, ext);

    temp_file = temp_file.with_extension(ext);
    let temp_stream = Stream {
        language: stream.language.clone(),
        media_type: stream.media_type.clone(),
        path: temp_file.clone(),
    };

    if temp_file.exists() && !config.no_resume {
        info!(
            "Saving [{}] {} (downloaded)",
            stream.media_type.to_string().green(),
            temp_file.to_string_lossy()
        );
        return Ok(temp_stream);
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
        let max_threads = config.max_threads as usize;
        let mut set: JoinSet<Result<(usize, Vec<u8>)>> = JoinSet::new();
        let mut results = vec![None; remaining.len()];

        for (i, segment) in remaining.iter().enumerate() {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            while set.len() >= max_threads {
                if let Some(Ok(result)) = set.join_next().await {
                    let (i, bytes) = match result {
                        Ok(v) => v,
                        Err(e) => {
                            set.abort_all();
                            return Err(e);
                        }
                    };
                    pb.update(bytes.len());
                    results[i] = Some(bytes);
                }
            }

            let url = base_url.join(&segment.uri)?;
            let mut request = config.client.get(url).query(&config.query);

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
                    return Err(e);
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

    if !running.load(Ordering::SeqCst) {
        return Err(Error::DownloadInterrupted);
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
            ttml_text_parser::parse_bytes(&data)
                .map_err(|e| Error::Other(e.to_string()))?
                .into_subtitles()
                .as_srt()
                .into_bytes()
        }
        _ => data,
    };

    File::create(&temp_file).await?.write_all(&output).await?;

    Ok(temp_stream)
}
