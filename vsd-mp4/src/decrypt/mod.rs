//! Decryption utilities for protected MP4 and HLS streams.

#[cfg(feature = "decrypt-hls")]
mod hls;

#[cfg(feature = "decrypt-cenc")]
mod cenc;

#[cfg(feature = "decrypt-cenc")]
mod cipher;

#[cfg(feature = "decrypt-cenc")]
mod reader;

#[cfg(feature = "decrypt-cenc")]
#[cfg_attr(docsrs, doc(cfg(feature = "decrypt-cenc")))]
pub use cenc::CencDecrypter;

#[cfg(feature = "decrypt-hls")]
#[cfg_attr(docsrs, doc(cfg(feature = "decrypt-hls")))]
pub use hls::{HlsAes128Decrypter, HlsSampleAesDecrypter};
