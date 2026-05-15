// This example shows using vsd as library in other rust project.
//
// [dependencies]
// vsd = { version = "0.5", default-features = false, features = ["rustls-tls"]}

use reqwest::Client;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use vsd::Result;
use vsd::{
    Downloader, Error, Muxer,
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
    let config = dl.config();

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
            let dl_info = match stream.download(&config, &running, Arc::new(Progress)).await {
                Ok(Some(info)) => info,
                Ok(None) => {
                    println!("Stream has no segments");
                    continue;
                }
                Err(Error::DownloadInterrupted) => {
                    println!("Download paused");
                    std::process::exit(0);
                }
                Err(e) => return Err(e),
            };

            println!("Downloaded {}", dl_info.path.to_string_lossy());
            muxer.0.push(dl_info);

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
