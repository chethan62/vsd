// This example shows using vsd as library in other rust project.
//
// [dependencies]
// vsd = { version = "0.5", default-features = false, features = ["rustls-tls"]}

use anyhow::Result;
use reqwest::Client;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use vsd::{
    Downloader, Muxer,
    playlist::MediaType,
    progress::{ProgressCallback, ProgressState},
};

struct Progress;

impl ProgressCallback for Progress {
    fn on_progress(&self, state: &ProgressState) {
        println!(
            "{}% ({}/{})",
            state.percent, state.downloaded_parts, state.total_parts
        );
    }

    fn on_finish(&self, state: &ProgressState) {
        println!(
            "{}% ({}/{})",
            state.percent, state.downloaded_parts, state.total_parts
        );
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let client = Client::new();
    let dl = Downloader::new(&client).no_resume(true);
    let config = dl.get_config();

    let mp = dl
        .parse(
            "https://media.axprod.net/TestVectors/Dash/not_protected_dash_1080p_h264/manifest.mpd",
            false,
        )
        .await?;

    // You can clone this var and pause download by setting its value to false.
    let running = Arc::new(AtomicBool::new(true));
    let mut muxer = Muxer(Vec::new());

    // Download first subtitle stream.
    for stream in mp.streams {
        if stream.media_type == MediaType::Subtitles {
            println!(
                "Downloading {} subtitles",
                stream.language.as_deref().unwrap_or("unknown")
            );

            // If stream is already downloaded then no progress updates will be triggered.
            let dl_info = stream
                .download(config, &running, Arc::new(Progress))
                .await?;

            let Some(dl_info) = dl_info else {
                println!("Stream has no segments");
                continue;
            };

            if dl_info.path.exists() {
                println!("Downloaded {}", dl_info.path.to_string_lossy());
                muxer.0.push(dl_info);
            } else {
                // Download must be paused using running var for this to happen.
                println!("Download paused");
            }

            break;
        }
    }

    println!("Muxing to output.srt");
    let ffmpeg = vsd::find_ffmpeg().unwrap();
    let output = PathBuf::from("output.srt");
    muxer.mux(&ffmpeg, &output, "srt").await?;
    muxer.clean(config.directory.as_deref()).await?;

    Ok(())
}
