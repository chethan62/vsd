use anyhow::Result;
use reqwest::Client;
use std::sync::{Arc, atomic::AtomicBool};
use vsd::{
    Downloader,
    playlist::MediaType,
    progress::{ProgressCallback, ProgressState},
};

struct Progress;

impl ProgressCallback for Progress {
    fn on_progress(&self, state: &ProgressState) {
        println!("{}%", state.percent);
    }

    fn on_finish(&self, state: &ProgressState) {
        println!("{}%", state.percent);
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

    let running = Arc::new(AtomicBool::new(true));

    for stream in mp.streams {
        if stream.media_type == MediaType::Subtitles {
            println!(
                "Downloading {} subtitles",
                stream.language.as_deref().unwrap_or("unknown")
            );

            let dl_info = stream
                .download(config, &running, Arc::new(Progress))
                .await?;

            if let Some(dl_info) = dl_info {
                println!("Downloaded {}", dl_info.path.to_string_lossy());
            }

            break;
        }
    }

    Ok(())
}
