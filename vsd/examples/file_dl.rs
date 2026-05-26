// This example shows using FileDownloader for multi-threaded file downloads.
//
// [dependencies]
// vsd = { version = "0.5", default-features = false, features = ["rustls-tls"]}

use std::sync::Arc;
use vsd::{
    FileDownloader, Result,
    progress::{ByteSize, Eta, ProgressCallback, ProgressState},
    reqwest::Client,
    tokio,
};

struct Progress;

impl ProgressCallback for Progress {
    fn on_progress(&self, state: &ProgressState) {
        println!(
            "{}% | {}/{} | {}/{} | {} | {}",
            state.percent,
            ByteSize(state.downloaded_bytes),
            ByteSize(state.estimated_bytes),
            state.downloaded_parts,
            state.total_parts,
            ByteSize(state.speed_bps as usize),
            Eta(state.eta_seconds),
        );
    }

    fn on_finish(&self, state: &ProgressState) {
        self.on_progress(state);
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let client = Client::new();

    FileDownloader::new(&client)
        .threads(5)
        .resume(true)
        .progress(Arc::new(Progress))
        .download(
            "https://github.com/llvm/llvm-project/releases/download/llvmorg-22.1.6/llvm-project-22.1.6.src.tar.xz",
            "target/llvm-project-22.1.6.src.tar.xz",
        )
        .await?;

    println!("Download complete!");
    Ok(())
}
