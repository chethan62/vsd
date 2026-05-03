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
    client: &Client,
    streams: &Vec<MediaPlaylist>,
    base_url: &Option<Url>,
    query: &Vec<(String, String)>,
    directory: Option<&PathBuf>,
    temp_files: &mut Streams,
    keys: &HashMap<String, String>,
) -> Result<()> {
    let total = streams.len();

    for stream in streams {
        if stream.media_type == MediaType::Subtitles {
            continue;
        }

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
            client,
            stream,
            base_url,
            query,
            &temp_file,
            keys,
            Progress::new(
                &format!("{}/{}", STREAM_DL_IDX.fetch_add(1, Ordering::SeqCst), total),
                stream.segments.len(),
            ),
        )
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_stream(
    client: &Client,
    stream: &MediaPlaylist,
    base_url: &Option<Url>,
    query: &Vec<(String, String)>,
    temp_file: &PathBuf,
    keys: &HashMap<String, String>,
    pb: Progress,
) -> Result<()> {
    let base_url = base_url.clone().unwrap_or(stream.uri.parse()?);
    let ext = stream.extension();
    let should_decrypt = !SKIP_DECRYPT.load(Ordering::SeqCst);
    let temp_dir = temp_file.with_extension("");
    let mut auto_increment_iv = false;
    let mut decrypter = Decrypter::None;

    let init_seg = stream
        .fetch_init(client, &base_url, query)
        .await?
        .map(Arc::new);

    let default_kid = if let Some(init_seg) = &init_seg {
        TencBox::from_init(init_seg)?.map(|x| x.default_kid_hex())
    } else {
        stream.default_kid()
    };

    fs::create_dir_all(&temp_dir).await?;

    let max_threads = MAX_THREADS.load(Ordering::SeqCst) as usize;
    let mut set: JoinSet<Result<usize>> = JoinSet::new();

    for (i, segment) in stream.segments.iter().enumerate() {
        while set.len() >= max_threads {
            if let Some(Ok(result)) = set.join_next().await {
                match result {
                    Ok(bytes) => pb.update(bytes),
                    Err(e) => {
                        RUNNING.store(false, Ordering::SeqCst);
                        error!("{}", e);
                        std::process::exit(1);
                    }
                }
            }
        }

        if should_decrypt {
            if decrypter.is_hls() && segment.key.is_none() && auto_increment_iv {
                decrypter.increment_iv();
            }

            if let Some(key) = &segment.key {
                let media_sequence = stream.media_sequence + i as u64;

                match key.method {
                    KeyMethod::Aes128 => {
                        decrypter = Decrypter::Aes128(HlsAes128Decrypter::new(
                            &key.key(client, &base_url, query).await?,
                            &key.iv(media_sequence)?,
                        ));
                        auto_increment_iv = key.iv.is_none();
                    }
                    KeyMethod::SampleAes => {
                        decrypter = Decrypter::SampleAes(HlsSampleAesDecrypter::new(
                            &key.key(client, &base_url, query).await?,
                            &key.iv(media_sequence)?,
                        ));
                        auto_increment_iv = key.iv.is_none();
                    }
                    KeyMethod::Cenc if !matches!(decrypter, Decrypter::Cenc(_)) => {
                        if keys.is_empty() {
                            bail!("Custom keys are required to proceed further.");
                        }

                        let default_kid = default_kid.as_ref().ok_or_else(|| {
                            anyhow!("Unable to determine default kid for this stream.")
                        })?;

                        let key = if let Some(v) = keys.get(default_kid) {
                            v.to_owned()
                        } else {
                            warn!(
                                "No key provided for '{}'; checking pssh data to identify other mappable kids.",
                                default_kid
                            );

                            let mut found = None;
                            if let Some(init_seg) = &init_seg {
                                for kid in PsshBox::from_init(init_seg)?
                                    .data
                                    .into_iter()
                                    .flat_map(|x| x.key_ids)
                                {
                                    if let Some(v) = keys.get(&kid.0) {
                                        found = Some(v.to_owned());
                                        break;
                                    }
                                }
                            }

                            found.ok_or_else(|| {
                                anyhow!("Unable to determine key for this stream.")
                            })?
                        };

                        info!("DrmKey [{}] {}:{}", "dec".magenta(), default_kid, key);
                        decrypter = Decrypter::Cenc(Arc::new(
                            CencDecryptingProcessor::builder()
                                .key(default_kid, &key)?
                                .build()?,
                        ));
                    }
                    _ => (),
                }
            }
        }

        let init_seg = init_seg.clone();
        let decrypter = decrypter.clone();
        let temp_file = temp_dir.join(format!("{}.{}.part", i, ext));
        let url = base_url.join(&segment.uri)?;
        let mut request = client.get(url.clone()).query(query);

        if let Some(range) = &segment.range {
            request = request.header(header::RANGE, range);
        }

        set.spawn(async move {
            let mut avl_tries = MAX_RETRIES.load(Ordering::SeqCst);
            let mut bytes;

            loop {
                match request.try_clone().unwrap().send().await {
                    Ok(response) => {
                        let status = response.status();

                        if status.is_success() {
                            bytes = response.bytes().await?.to_vec();
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

            // Trim fake PNG header.
            if bytes.len() >= 8 && bytes[..8] == PNG_HEADER {
                bytes = bytes.split_off(8)
            }

            let bytes = decrypter.decrypt(bytes, init_seg.as_deref().map(|x| x.as_ref()))?;

            let mut f = File::create(&temp_file).await?;

            if let Some(init_seg) = &init_seg {
                f.write_all(init_seg).await?;
            }

            f.write_all(&bytes).await?;
            f.flush().await?;
            fs::rename(&temp_file, temp_file.with_extension("")).await?;

            Ok(size)
        });
    }

    while let Some(Ok(result)) = set.join_next().await {
        match result {
            Ok(bytes) => pb.update(bytes),
            Err(e) => {
                RUNNING.store(false, Ordering::SeqCst);
                error!("{}", e);
                std::process::exit(1);
            }
        }
    }

    eprintln!();

    if SKIP_MERGE.load(Ordering::SeqCst) {
        debug!("Stream merging skipped.");
    } else {
        info!(
            "Mergin [{}] {}",
            stream.media_type.to_string().cyan(),
            temp_file.to_string_lossy()
        );

        let mut outfile = File::create(temp_file).await?;

        for i in 0..stream.segments.len() {
            let path = temp_dir.join(format!("{}.{}", i, ext));

            if path.exists() {
                io::copy(&mut File::open(&path).await?, &mut outfile).await?;
                // trace!("Deleting '{}' file.", path.to_string_lossy());
                // fs::remove_file(&path).await?;
            }
        }

        debug!("Deleting '{}' directory.", temp_dir.to_string_lossy());
        // tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        fs::remove_dir_all(&temp_dir).await?;
    }
    Ok(())
}
