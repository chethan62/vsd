use crate::{
    core::{DownloadConfig, mux::Muxer, sub, vid},
    error::Result,
    playlist::{MediaPlaylist, MediaType},
    progress::Progress,
};
use colored::Colorize;
use log::{info, warn};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use vsd_mp4::{boxes::TencBox, pssh::PsshBox};

pub async fn download_streams(
    config: &DownloadConfig,
    streams: Vec<MediaPlaylist>,
) -> Result<Muxer> {
    if !config.skip_decrypt {
        dump_pssh_info(config, &streams).await?;
    }

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
            "DownLd [{}] {}",
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

pub async fn dump_pssh_info(config: &DownloadConfig, streams: &[MediaPlaylist]) -> Result<()> {
    let mut default_kids = HashSet::new();
    let mut init_segments = Vec::new();

    for stream in streams {
        let Some(bytes) = stream.fetch_init(config).await? else {
            continue;
        };

        if let Some(kid) = TencBox::from_init(&bytes)?.and_then(|x| {
            let kid = x.default_kid_hex();
            if kid == "00000000000000000000000000000000" {
                stream.default_kid()
            } else {
                Some(kid)
            }
        }) {
            default_kids.insert(kid);
        }

        init_segments.push(bytes);
    }

    let mut seen = HashSet::new();

    for bytes in &init_segments {
        let pssh = PsshBox::from_init(bytes)?;

        for pssh in pssh.boxes {
            let pssh_base64 = pssh.as_base64();
            if !seen.insert(pssh_base64.clone()) {
                continue;
            }

            info!(
                "DrmPsh [{}] {}",
                pssh.system_id.to_string().magenta(),
                pssh_base64,
            );
            for kid in &pssh.key_ids {
                info!(
                    "DrmKid [{}] {}{}",
                    pssh.system_id.to_string().magenta(),
                    kid,
                    if default_kids.contains(kid) {
                        " (required)".bold().red()
                    } else {
                        "".normal()
                    },
                );
            }
        }
    }

    Ok(())
}
