// This example shows using vsd as library in other rust project.
//
// [dependencies]
// vsd = { version = "0.5", default-features = false, features = ["rustls-tls"]}

use std::sync::Arc;
use vsd::{
    Error, Muxer, PlaylistDownloader, Result,
    playlist::MediaType,
    progress::{ByteSize, Eta, ProgressCallback, ProgressState},
    reqwest::Client,
    tokio,
    tokio_util::sync::CancellationToken,
};

struct Progress;

impl ProgressCallback for Progress {
    fn on_progress(&self, state: &ProgressState) {
        println!(
            "{}% | {}/~{} | {}/{} | {}",
            state.percent,
            ByteSize(state.downloaded_bytes),
            ByteSize(state.estimated_bytes),
            state.downloaded_parts,
            state.total_parts,
            Eta(state.eta_seconds)
        );
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let client = Client::new();
    let dl = PlaylistDownloader::new(&client).no_resume(true);
    let config = dl.config();

    let mp = dl
        .parse(
            "https://media.axprod.net/TestVectors/Dash/not_protected_dash_1080p_h264/manifest.mpd",
            false,
        )
        .await?;

    // You can clone this token and call .cancel() to pause a download.
    let token = CancellationToken::new();
    let mut muxer = Muxer::new();

    // Download first subtitle stream.
    for stream in mp.streams {
        if stream.media_type == MediaType::Subtitles {
            println!(
                "Downloading {} subtitles",
                stream.language.as_deref().unwrap_or("unknown")
            );

            // If stream is already downloaded then no progress updates will be triggered.
            let dl_info = match stream.download(&config, Arc::new(Progress), &token).await {
                Ok(info) => info,
                Err(Error::UnsupportedEncryption(e)) => {
                    println!("Unsupported encryption {}", e);
                    continue;
                }
                Err(Error::MissingSegments) => {
                    println!("Stream has no segments");
                    continue;
                }
                Err(Error::MissingKey(key_id)) => {
                    println!("Missing decryption key for {key_id}");
                    continue;
                }
                Err(Error::DownloadInterrupted) => {
                    println!("Download paused");
                    std::process::exit(0);
                }
                Err(e) => return Err(e),
            };

            println!("Downloaded {}", dl_info.path.to_string_lossy());
            muxer.push(dl_info);

            break;
        }
    }

    println!("Muxing to output.srt");
    muxer
        .mux(&vsd::find_ffmpeg().unwrap(), "output.srt", "srt")
        .await?;
    muxer.clean(config.directory.as_deref()).await?;

    Ok(())
}
