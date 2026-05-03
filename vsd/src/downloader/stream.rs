use crate::{
    downloader::{
        MAX_RETRIES, MAX_THREADS, RUNNING, SKIP_DECRYPT, SKIP_MERGE, STREAM_DL_IDX,
        encryption::Decrypter,
        mux::{Stream, Streams},
    },
    playlist::{KeyMethod, MediaPlaylist, MediaType},
    progress::Progress,
};
use anyhow::{Result, anyhow, bail};
use colored::Colorize;
use log::{debug, error, info, trace, warn};
use reqwest::{Client, StatusCode, Url, header};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tokio::{
    fs::{self, File},
    io::{self, AsyncWriteExt},
    task::JoinSet,
};
use vsd_mp4::{
    boxes::TencBox,
    decrypt::{CencDecryptingProcessor, HlsAes128Decrypter, HlsSampleAesDecrypter},
    pssh::PsshBox,
};

const PNG_HEADER: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

#[allow(clippy::too_many_arguments)]
pub async fn download_streams(
    base_url: &Option<Url>,
    client: &Client,
    directory: Option<&PathBuf>,
    keys: &HashMap<String, String>,
    query: &Vec<(String, String)>,
    streams: &Vec<MediaPlaylist>,
    temp_files: &mut Streams,
) -> Result<()> {
    let total = streams.len();
    let streams = streams
        .into_iter()
        .filter(|x| x.media_type != MediaType::Subtitles)
        .collect::<Vec<_>>();

    for stream in streams {
        info!(
            "DownLD [{}] {}",
            stream.media_type.to_string().green(),
            stream.display().cyan(),
        );

        if stream.segments.is_empty() {
            warn!("Stream skipped because no segments were found.");
            continue;
        }

        let temp_file = stream.path(directory);

        temp_files.0.push(Stream {
            language: stream.language.clone(),
            media_type: stream.media_type.clone(),
            path: temp_file.clone(),
        });

        info!(
            "Saving [{}] {}",
            stream.media_type.to_string().green(),
            temp_file.with_extension("").to_string_lossy()
        );
        download_stream(
            base_url,
            client,
            keys,
            Progress::new(
                &format!("{}/{}", STREAM_DL_IDX.fetch_add(1, Ordering::SeqCst), total),
                stream.segments.len(),
            ),
            query,
            stream,
            &temp_file,
        )
        .await?;
    }

    // eprintln!();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_stream(
    base_url: &Option<Url>,
    client: &Client,
    keys: &HashMap<String, String>,
    pb: Progress,
    query: &Vec<(String, String)>,
    stream: &MediaPlaylist,
    temp_file: &PathBuf,
) -> Result<()> {
    let base_url = base_url
        .clone()
        .unwrap_or(stream.uri.parse::<Url>().unwrap());
    let mut decrypter = Decrypter::None;
    let temp_dir = temp_file.with_extension("");
    let extension = stream.extension();
    let total = stream.segments.len();
    let should_decrypt = !SKIP_DECRYPT.load(Ordering::SeqCst);
    let mut increment_media_sequence = false;
    let mut media_sequence = stream.media_sequence;
    let media_type = stream.media_type.to_string();
    let init_seg = stream.fetch_init_seg(client, &base_url, query).await?;

    let default_kid = if let Some(init_seg) = &init_seg {
        TencBox::from_init(init_seg)?.map(|x| x.default_kid_hex())
    } else {
        stream.default_kid()
    };

    fs::create_dir_all(&temp_dir).await?;

    let max_threads = MAX_THREADS.load(Ordering::SeqCst) as usize;
    let mut set: JoinSet<usize> = JoinSet::new();

    for (i, segment) in stream.segments.iter().enumerate() {
        while set.len() >= max_threads {
            if let Some(Ok(bytes)) = set.join_next().await {
                pb.update(bytes);
            }
        }

        if should_decrypt {
            if decrypter.is_hls() && segment.key.is_none() && increment_media_sequence {
                decrypter.increment_iv();
                media_sequence += 1;
            }

            if let Some(key) = &segment.key {
                match key.method {
                    KeyMethod::Aes128 | KeyMethod::SampleAes => {
                        match key.method {
                            KeyMethod::Aes128 => {
                                decrypter = Decrypter::Aes128(HlsAes128Decrypter::new(
                                    &key.key(&base_url, client, query).await?,
                                    &key.iv(media_sequence)?,
                                ));
                            }
                            KeyMethod::SampleAes => {
                                decrypter = Decrypter::SampleAes(HlsSampleAesDecrypter::new(
                                    &key.key(&base_url, client, query).await?,
                                    &key.iv(media_sequence)?,
                                ));
                            }
                            _ => (),
                        }

                        if key.iv.is_none() {
                            increment_media_sequence = true;
                            media_sequence += 1;
                        } else {
                            increment_media_sequence = false;
                        }
                    }
                    KeyMethod::Cenc => {
                        if keys.is_empty() {
                            bail!("Custom keys are required to proceed further.");
                        }

                        let default_kid = default_kid.as_ref().ok_or_else(|| {
                            anyhow!("Unable to determine the default KID for this stream.")
                        })?;

                        let mut key = None;

                        if keys.contains_key(default_kid) {
                            key = Some(keys.get(default_kid).unwrap().to_owned())
                        } else {
                            warn!(
                                "No key provided for ({}:?); checking PSSH data to identify other mappable KIDs.",
                                default_kid
                            );

                            if let Some(init_seg) = &init_seg {
                                for kid in PsshBox::from_init(init_seg)?
                                    .data
                                    .into_iter()
                                    .map(|x| x.key_ids)
                                    .flatten()
                                {
                                    if keys.contains_key(&kid.0) {
                                        key = Some(keys.get(&kid.0).unwrap().to_owned());
                                    }
                                }
                            }
                        }

                        let key = key.ok_or_else(|| {
                            anyhow!("Unable to determine the key for this stream.")
                        })?;

                        decrypter = Decrypter::Cenc(Arc::new(
                            CencDecryptingProcessor::builder()
                                .key(default_kid, &key)?
                                .build()?,
                        ));

                        info!("DrmKey [{}] {}:{}", "dec".magenta(), default_kid, key);
                    }
                    _ => (),
                }
            }
        }

        let init_seg = init_seg.clone();
        let decrypter = decrypter.clone();
        let temp_file = temp_dir.join(format!("{}.{}.part", i, extension));
        let url = base_url.join(&segment.uri)?;
        let mut request = client.get(url.clone()).query(query);

        if let Some(range) = &segment.range {
            request = request.header(header::RANGE, range);
        }

        set.spawn(async move {
            let mut avl_tries = MAX_RETRIES.load(Ordering::SeqCst);
            let bytes;

            loop {
                match request.try_clone().unwrap().send().await {
                    Ok(response) => {
                        let status = response.status();

                        if status.is_success() {
                            bytes = response.bytes().await.unwrap().to_vec();
                            break;
                        }

                        if avl_tries == 0 {
                            RUNNING.store(false, Ordering::SeqCst);
                            error!(
                                "{} request failed ({}): '{}'",
                                url,
                                status,
                                response.text().await.unwrap(),
                            );
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        if avl_tries == 0 {
                            RUNNING.store(false, Ordering::SeqCst);
                            error!(
                                "{} request failed ({})",
                                url,
                                e.status().unwrap_or(StatusCode::NOT_FOUND)
                            );
                            std::process::exit(1);
                        }
                    }
                }

                trace!("Retrying {}", url);
                avl_tries -= 1;
            }

            let size = bytes.len();
            let bytes = trim_fake_png_header(bytes);
            let bytes = decrypter.decrypt(bytes, init_seg.clone()).unwrap();

            let mut f = File::create(&temp_file).await.unwrap();

            if let Some(init_seg) = init_seg {
                f.write_all(&init_seg).await.unwrap();
            }

            f.write_all(&bytes).await.unwrap();
            f.flush().await.unwrap();
            fs::rename(&temp_file, temp_file.with_extension(""))
                .await
                .unwrap();

            size
        });
    }

    if let Some(Ok(bytes)) = set.join_next().await {
        pb.update(bytes);
    }

    eprintln!();

    if SKIP_MERGE.load(Ordering::SeqCst) {
        debug!("Stream merging skipped.");
    } else {
        info!(
            "Mergin [{}] {}",
            media_type.cyan(),
            temp_file.to_string_lossy()
        );

        let mut outfile = File::create(temp_file).await?;

        for i in 0..total {
            let path = temp_dir.join(format!("{}.{}", i, extension));

            if path.exists() {
                io::copy(&mut File::open(&path).await?, &mut outfile).await?;
            }
        }

        debug!("Deleting '{}' directory.", temp_dir.to_string_lossy());
        fs::remove_dir_all(&temp_dir).await?;
    }
    Ok(())
}

fn trim_fake_png_header(mut data: Vec<u8>) -> Vec<u8> {
    if data.len() >= 8 && data[0..8] == PNG_HEADER {
        data.drain(0..8);
        data
    } else {
        data
    }
}
