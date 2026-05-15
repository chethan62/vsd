use crate::{
    core::{DownloadConfig, enc::Decrypter, mux::Stream},
    playlist::{KeyMethod, MediaPlaylist},
    progress::Progress,
};
use anyhow::{Result, anyhow, bail};
use colored::Colorize;
use log::{debug, info, trace, warn};
use reqwest::{StatusCode, header};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
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

pub async fn download(
    config: &DownloadConfig,
    running: &AtomicBool,
    pb: Progress,
    stream: &MediaPlaylist,
) -> Result<Stream> {
    let temp_file = stream.path(config.directory.as_ref());
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
            temp_file.with_extension("").to_string_lossy()
        );
    }

    let base_url = stream.uri.parse()?;
    let ext = stream.extension();
    let pb_handle = pb.spawn();
    let should_decrypt = !config.skip_decrypt;
    let temp_dir = temp_file.with_extension("");
    let mut auto_increment_iv = false;
    let mut decrypter = Decrypter::None;

    let init = stream
        .fetch_init(&config.client, &base_url, &config.query)
        .await?
        .map(Arc::new);

    let default_kid = if let Some(init) = &init {
        TencBox::from_init(init)?.map(|x| x.default_kid_hex())
    } else {
        stream.default_kid()
    };

    if config.no_resume && temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).await?;
    }
    fs::create_dir_all(&temp_dir).await?;

    if let Some(init) = &init {
        let mut f = File::create(temp_dir.join("init.mp4")).await?;
        f.write_all(init).await?;
        f.flush().await?;
    }

    let max_threads = config.max_threads as usize;
    let mut set: JoinSet<Result<usize>> = JoinSet::new();

    for (i, segment) in stream.segments.iter().enumerate() {
        if !running.load(Ordering::SeqCst) {
            break;
        }

        while set.len() >= max_threads {
            if let Some(Ok(result)) = set.join_next().await {
                match result {
                    Ok(bytes) => pb.update(bytes),
                    Err(e) => {
                        set.abort_all();
                        bail!(e);
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
                            &key.key(&config.client, &base_url, &config.query).await?,
                            &key.iv(media_sequence)?,
                        ));
                        auto_increment_iv = key.iv.is_none();
                    }
                    KeyMethod::SampleAes => {
                        decrypter = Decrypter::SampleAes(HlsSampleAesDecrypter::new(
                            &key.key(&config.client, &base_url, &config.query).await?,
                            &key.iv(media_sequence)?,
                        ));
                        auto_increment_iv = key.iv.is_none();
                    }
                    KeyMethod::Cenc if !matches!(decrypter, Decrypter::Cenc(_)) => {
                        if config.keys.is_empty() {
                            bail!("Custom keys are required to proceed further.");
                        }

                        let default_kid = default_kid.as_ref().ok_or_else(|| {
                            anyhow!("Unable to determine default kid for this stream.")
                        })?;

                        let key = if let Some(v) = config.keys.get(default_kid) {
                            v.to_owned()
                        } else {
                            warn!(
                                "No key provided for '{}'; checking pssh data to identify other mappable kids.",
                                default_kid
                            );

                            let mut found = None;
                            if let Some(init) = &init {
                                for kid in PsshBox::from_init(init)?
                                    .data
                                    .into_iter()
                                    .flat_map(|x| x.key_ids)
                                {
                                    if let Some(v) = config.keys.get(&kid.0) {
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

        // Resume Logic
        let temp_file = temp_dir.join(format!("{}.{}.part", i, ext));
        let out_file = temp_file.with_extension("");

        if out_file.exists() {
            let size = fs::metadata(&out_file).await?.len();
            pb.skip(size as usize);
            continue;
        }

        let init = init.clone();
        let decrypter = decrypter.clone();
        let url = base_url.join(&segment.uri)?;
        let mut request = config.client.get(url.clone()).query(&config.query);

        if let Some(range) = &segment.range {
            request = request.header(header::RANGE, range);
        }

        let max_retries = config.max_retries;

        set.spawn(async move {
            let mut avl_tries = max_retries;
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
                            bail!(
                                "{} request failed ({}): '{}'",
                                url,
                                status,
                                response.text().await?,
                            );
                        }
                    }
                    Err(e) => {
                        if avl_tries == 0 {
                            bail!(
                                "{} request failed ({})",
                                url,
                                e.status().unwrap_or(StatusCode::NOT_FOUND)
                            );
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

            let bytes = decrypter.decrypt(bytes, init.as_deref().map(|x| x.as_ref()))?;

            let mut f = File::create(&temp_file).await?;
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
                set.abort_all();
                bail!(e);
            }
        }
    }

    pb_handle.abort();
    pb.finish();

    if !running.load(Ordering::SeqCst) {
        warn!("Download interrupted.");
        std::process::exit(0);
    }

    if config.skip_merge {
        debug!("Stream merging skipped.");
    } else {
        info!(
            "Mergin [{}] {}",
            stream.media_type.to_string().cyan(),
            temp_file.to_string_lossy()
        );

        let mut output = File::create(temp_file).await?;
        let init_path = temp_dir.join("init.mp4");

        if init_path.exists() {
            io::copy(&mut File::open(&init_path).await?, &mut output).await?;
        }

        for i in 0..stream.segments.len() {
            let path = temp_dir.join(format!("{}.{}", i, ext));

            if path.exists() {
                io::copy(&mut File::open(&path).await?, &mut output).await?;
            }
        }

        debug!("Deleting '{}' directory.", temp_dir.to_string_lossy());
        fs::remove_dir_all(&temp_dir).await?;
    }

    Ok(temp_stream)
}
