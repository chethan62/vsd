//! A command-line utility and library for downloading HLS and DASH video streams.
//!
//! `vsd` enables concurrent chunk/segment downloading, automatic decryption of AES-128
//! and Sample-AES encrypted streams, PSSH DRM metadata extraction, and automated muxing
//! using ffmpeg.
//!
//! # Cargo Features
//!
//! The following Cargo features can be enabled or disabled:
//!
//! | Feature | Description |
//! |-------|-------------|
//! | `capture` (*default*) | Enables the `capture` sub-command. |
//! | `license` (*default*) | Enables the `license` sub-command. |
//! | `rustls-tls` (*default*) | Enables the `rustls` TLS backend for the [reqwest] crate. |
//! | `native-tls` | Enables the `native-tls` TLS backend for the [reqwest] crate. |
//! | `native-tls-vendored` | Enables the `native-tls-vendored` TLS backend for the [reqwest] crate. |
//!
//! # Examples
//!
//! Below are examples demonstrating how to use the library to download files and playlists.
//!
//! ## `examples/playlist_dl.rs`
//!
//! ```rust,no_run
#![doc = include_str!("../examples/playlist_dl.rs")]
//! ```
//!
//! ## `examples/file_dl.rs`
//!
//! ```rust,no_run
#![doc = include_str!("../examples/file_dl.rs")]
//! ```
//!
//! [reqwest]: https://docs.rs/reqwest/latest/reqwest/#optional-features

#[macro_use]
mod error;

mod cli;
mod core;
mod dash;
mod format;
mod hls;
mod logger;
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
