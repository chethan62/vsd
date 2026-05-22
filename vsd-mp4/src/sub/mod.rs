//! Subtitle extraction and parsing utilities for MP4 streams.

mod builder;

pub use builder::Subtitles;

#[cfg(feature = "sub-ttml")]
mod stpp;

#[cfg(feature = "sub-ttml")]
#[cfg_attr(docsrs, doc(cfg(feature = "sub-ttml")))]
pub use stpp::StppSubsParser;

#[cfg(feature = "sub-vtt")]
mod wvtt;

#[cfg(feature = "sub-vtt")]
#[cfg_attr(docsrs, doc(cfg(feature = "sub-vtt")))]
pub use wvtt::WvttSubsParser;

#[cfg(feature = "sub-ttml")]
#[cfg_attr(docsrs, doc(cfg(feature = "sub-ttml")))]
pub mod ttml;
