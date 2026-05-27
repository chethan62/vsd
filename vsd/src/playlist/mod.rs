//! Common types used for parsing, representing, and describing HLS and DASH playlists.

mod master;
mod media;
mod other;
mod types;

pub(crate) use master::ClipRange;

pub use types::{
    Key, KeyMethod, Map, MasterPlaylist, MediaPlaylist, MediaType, PlaylistType, Range, Segment,
    StreamMetadata,
};
