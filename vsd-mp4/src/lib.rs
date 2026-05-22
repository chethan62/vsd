#![cfg_attr(docsrs, feature(doc_cfg))]

//! This crate contains an MP4 parser ported from the [shaka-player](https://github.com/shaka-project/shaka-player) project.
//!
//! It also includes optional features for decryption, parsing subtitles, and processing `PSSH` and `SIDX` boxes.
//!
//! # Optional Features
//!
//! The following Cargo features can be enabled or disabled (all features are enabled by default):
//!
//! | Feature            | Description                                                                   |
//! | :---               | :---                                                                          |
//! | **`decrypt-cenc`** | Enables support for Common Encryption (`CENC`) scheme decryption.             |
//! | **`decrypt-hls`**  | Enables support for HTTP Live Streaming (`HLS`) segment decryption.           |
//! | **`pssh`**         | Enables support for parsing Protection System Specific Header (`PSSH`) boxes. |
//! | **`sidx`**         | Enables support for parsing Segment Index (`SIDX`) boxes.                     |
//! | **`sub-ttml`**     | Enables support for extracting subtitles from `STPP` boxes.                   |
//! | **`sub-vtt`**      | Enables support for extracting subtitles from `WVTT` boxes.                   |

pub mod boxes;

#[cfg(any(feature = "decrypt-cenc", feature = "decrypt-hls"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "decrypt-cenc", feature = "decrypt-hls")))
)]
pub mod decrypt;

#[cfg(feature = "pssh")]
#[cfg_attr(docsrs, doc(cfg(feature = "pssh")))]
pub mod pssh;

#[cfg(any(feature = "sub-ttml", feature = "sub-vtt"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "sub-ttml", feature = "sub-vtt"))))]
pub mod sub;

mod error;
mod parser;
mod reader;

pub use error::{Error, Result};
pub use parser::*;
pub use reader::Reader;
