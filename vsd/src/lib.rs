#[macro_use]
mod error;

mod cli;
mod cookie;
mod core;
mod dash;
mod hls;
mod logger;
mod select;
mod utils;

pub mod playlist;
pub mod progress;

#[doc(hidden)]
pub use cli::Args;

pub use core::{DownloadConfig, Downloader, Muxer, Stream};
pub use error::{Error, Result};
pub use reqwest;
pub use tokio;
pub use tokio_util;
pub use utils::find_ffmpeg;
pub use vsd_mp4;
