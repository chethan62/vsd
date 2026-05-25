use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MasterPlaylist {
    pub playlist_type: PlaylistType,
    pub uri: String,
    pub streams: Vec<MediaPlaylist>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MediaPlaylist {
    pub bandwidth: Option<u64>,
    pub channels: Option<f32>,
    pub codecs: Option<String>,
    pub extension: Option<String>,
    pub frame_rate: Option<f32>,
    pub id: String,
    pub i_frame: bool,
    pub language: Option<String>,
    pub live: bool,
    pub media_sequence: u64,
    pub media_type: MediaType,
    pub playlist_type: PlaylistType,
    pub resolution: Option<(u64, u64)>,
    pub segments: Vec<Segment>,
    pub uri: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Segment {
    pub duration: f32,
    pub key: Option<Key>,
    pub map: Option<Map>,
    pub range: Option<Range>,
    pub uri: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Key {
    pub default_kid: Option<String>,
    pub iv: Option<String>,
    pub method: KeyMethod,
    pub uri: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Map {
    pub range: Option<Range>,
    pub uri: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Range(pub u64, pub u64);

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum MediaType {
    Video,
    Audio,
    Subtitles,
    #[default]
    Undefined,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum PlaylistType {
    Dash,
    #[default]
    Hls,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum KeyMethod {
    Aes128,
    Cenc,
    #[default]
    None,
    Other(String),
    SampleAes,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct StreamMetadata {
    pub bandwidth: Option<u64>,
    pub channels: Option<f32>,
    pub codecs: Option<String>,
    pub default_kid: Option<String>,
    pub encryption_type: KeyMethod,
    pub frame_rate: Option<f32>,
    pub index: usize,
    pub language: Option<String>,
    pub media_type: MediaType,
    pub playlist_type: PlaylistType,
    pub pssh: HashSet<String>,
    pub resolution: Option<(u64, u64)>,
}
