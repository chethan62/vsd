use crate::{
    core::{self, PlaylistDownloadConfig, Stream},
    error::{Error, Result},
    playlist::types::{MediaPlaylist, MediaType, PlaylistType, Segment},
    progress::{ByteSize, Progress, ProgressCallback},
    utils,
};
use log::debug;
use reqwest::header;
use std::{fmt::Display, path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;
use url::Url;

impl MediaPlaylist {
    pub(crate) fn extension(&self) -> &str {
        if let Some(ext) = &self.extension {
            return ext;
        }

        if let Some(first) = self.segments.first() {
            let is_mp4 = |uri: &str| {
                let path = uri.split_once('?').map_or(uri, |(p, _)| p);
                path.ends_with(".mp4") || path.ends_with(".m4s")
            };

            if is_mp4(&first.uri) || first.map.as_ref().is_some_and(|m| is_mp4(&m.uri)) {
                return "mp4";
            }
        }

        match self.playlist_type {
            PlaylistType::Hls => "ts",
            PlaylistType::Dash => "mp4",
        }
    }

    pub(crate) fn path(&self, directory: Option<&PathBuf>) -> PathBuf {
        let filename = format!("vsd-{}-{}.{}", self.media_type, self.id, self.extension());
        directory
            .map(|d| d.join(&filename))
            .unwrap_or_else(|| PathBuf::from(filename))
    }

    pub fn default_kid(&self) -> Option<String> {
        self.segments
            .first()
            .and_then(|s| s.key.as_ref())
            .and_then(|k| k.default_kid.as_ref())
            .map(|kid| kid.to_ascii_lowercase().replace('-', ""))
    }

    pub async fn download(
        &self,
        config: &PlaylistDownloadConfig,
        progress: Arc<dyn ProgressCallback>,
        token: &CancellationToken,
    ) -> Result<Stream> {
        if self.segments.is_empty() {
            return Err(Error::MissingSegments);
        }

        let progress = Progress::new(&self.id, self.segments.len(), Some(progress));
        let temp_file = if self.media_type == MediaType::Subtitles {
            core::sub::download(config, progress, token, self).await?
        } else {
            core::vid::download(config, progress, token, self).await?
        };

        Ok(temp_file)
    }

    pub async fn fetch_init(&self, config: &PlaylistDownloadConfig) -> Result<Option<Vec<u8>>> {
        let Some(Segment { map: Some(map), .. }) = self.segments.first() else {
            return Ok(None);
        };

        let url = self.uri.parse::<Url>()?.join(&map.uri)?;
        let mut request = config.client.get(url.clone()).query(&*config.query);

        if let Some(range) = &map.range {
            request = request.header(header::RANGE, range);
        }

        debug!(
            "Fetching {} (init@{})",
            url,
            map.range
                .as_ref()
                .map(|x| format!("{}-{}", x.0, x.1))
                .as_deref()
                .unwrap_or("full-range")
        );
        let response = request.send().await?;
        let bytes = utils::fetch_bytes(response).await?;
        Ok(Some(bytes))
    }
}

impl Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Video => "vid",
                Self::Audio => "aud",
                Self::Subtitles => "sub",
                Self::Undefined => "und",
            }
        )
    }
}

impl MediaPlaylist {
    fn truncate(s: &str, width: usize) -> String {
        if s.chars().count() > width {
            let mut truncated = s.chars().take(width - 1).collect::<String>();
            truncated.push('…');
            truncated
        } else {
            s.to_owned()
        }
    }

    fn fmt_resolution(&self) -> String {
        self.resolution
            .map(|(w, h)| {
                match (w, h) {
                    (256, 144) => "144p",
                    (426, 240) => "240p",
                    (640, 360) => "360p",
                    (854, 480) => "480p",
                    (1280, 720) => "720p",
                    (1920, 1080) => "1080p",
                    (2048, 1080) => "2K",
                    (2560, 1440) => "1440p",
                    (3840, 2160) => "4K",
                    (7680, 4320) => "8K",
                    _ => return format!("{w}x{h}"),
                }
                .into()
            })
            .unwrap_or_else(|| "?".into())
    }

    fn fmt_bandwidth(&self) -> String {
        self.bandwidth
            .map(|b| ByteSize(b as usize).to_string())
            .unwrap_or_else(|| "?".into())
    }

    fn fmt_codecs(&self) -> String {
        Self::truncate(self.codecs.as_deref().unwrap_or("?"), 10)
    }

    fn fmt_language(&self) -> String {
        Self::truncate(self.language.as_deref().unwrap_or("?"), 9)
    }

    pub fn display(&self) -> String {
        self.to_string()
            .split('|')
            .map(|x| x.replace(" ", ""))
            .collect::<Vec<String>>()
            .join(" ")
    }
}

impl std::fmt::Display for MediaPlaylist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.media_type {
            MediaType::Video => {
                write!(
                    f,
                    "{:>9} | {:>9} | {:>10} | {} fps",
                    self.fmt_resolution(),
                    self.fmt_bandwidth(),
                    self.fmt_codecs(),
                    self.frame_rate.map_or("?".into(), |r| r.to_string())
                )?;
                if self.live {
                    write!(f, " | live")?;
                }
                if self.i_frame {
                    write!(f, " | iframe")?;
                }
            }
            MediaType::Audio => {
                write!(
                    f,
                    "{:>9} | {:>9} | {:>10} | {} ch",
                    self.fmt_language(),
                    self.fmt_bandwidth(),
                    self.fmt_codecs(),
                    self.channels.map_or("?".into(), |c| c.to_string())
                )?;
                if self.live {
                    write!(f, " | live")?;
                }
            }
            MediaType::Subtitles => {
                write!(
                    f,
                    "{:>9} | {:>9} | {:>10} |",
                    self.fmt_language(),
                    "?KiB",
                    self.fmt_codecs()
                )?;
            }
            MediaType::Undefined => {
                write!(f, "{}", self.uri)?;
            }
        }
        Ok(())
    }
}
