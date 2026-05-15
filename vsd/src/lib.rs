#[macro_use]
pub mod error;

mod cli;
mod cookie;
mod core;
mod dash;
mod hls;
mod logger;
mod options;
mod selector;
mod utils;

pub mod playlist;
pub mod progress;

#[doc(hidden)]
pub use cli::Args;

pub use core::{DownloadConfig, Downloader, Muxer, Stream};
pub use error::{Error, Result};
pub use reqwest;
pub use utils::find_ffmpeg;
