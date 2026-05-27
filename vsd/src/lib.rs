#![doc = "A command-line utility and library for downloading HLS and DASH video streams."]
#![doc = ""]
#![doc = "`vsd` enables concurrent chunk/segment downloading, automatic decryption of AES-128"]
#![doc = "and Sample-AES encrypted streams, PSSH DRM metadata extraction, and automated muxing"]
#![doc = "using ffmpeg."]
#![doc = ""]
#![doc = "# Cargo Features"]
#![doc = ""]
#![doc = "The following Cargo features can be enabled or disabled:"]
#![doc = ""]
#![doc = "| Feature | Description |"]
#![doc = "|-------|-------------|"]
#![doc = "| `capture` (*default*) | Enables the `capture` sub-command. |"]
#![doc = "| `license` (*default*) | Enables the `license` sub-command. |"]
#![doc = "| `rustls-tls` (*default*) | Enables the `rustls` TLS backend for the [reqwest] crate. |"]
#![doc = "| `native-tls` | Enables the `native-tls` TLS backend for the [reqwest] crate. |"]
#![doc = "| `native-tls-vendored` | Enables the `native-tls-vendored` TLS backend for the [reqwest] crate. |"]
#![doc = ""]
#![doc = "# Examples"]
#![doc = ""]
#![doc = "Below are examples demonstrating how to use the library to download files and playlists."]
#![doc = ""]
#![doc = "## `examples/playlist_dl.rs`"]
#![doc = ""]
#![doc = "```rust,no_run"]
#![doc = include_str!("../examples/playlist_dl.rs")]
#![doc = "```"]
#![doc = ""]
#![doc = "## `examples/file_dl.rs`"]
#![doc = ""]
#![doc = "```rust,no_run"]
#![doc = include_str!("../examples/file_dl.rs")]
#![doc = "```"]
#![doc = ""]
#![doc = "[reqwest]: https://docs.rs/reqwest/latest/reqwest/#optional-features"]

#[macro_use]
mod error;

mod cli;
mod core;
mod dash;
mod hls;
mod logger;
mod select;
mod utils;

pub mod cookie;
pub mod playlist;
pub mod progress;

#[doc(hidden)]
pub use cli::Args;

pub use core::{FileDownloader, Muxer, PlaylistDownloadConfig, PlaylistDownloader, Stream};
pub use error::{Error, Result};
pub use reqwest;
pub use tokio;
pub use tokio_util;
pub use utils::{find_ffmpeg, gen_id};
pub use vsd_mp4;
