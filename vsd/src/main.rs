use clap::Parser;
use log::error;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    if let Err(e) = vsd::Args::parse().execute().await {
        error!("{}", e);
        std::process::exit(1);
    }
}
