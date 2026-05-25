use crate::{
    core::DownloadConfig,
    error::Result,
    playlist::types::{MasterPlaylist, MediaType, StreamMetadata},
    select::{SelectFilters, SelectType, StreamSelector},
};
use std::{cmp::Reverse, collections::HashSet};
use vsd_mp4::{boxes::TencBox, pssh::PsshBox};

impl MasterPlaylist {
    pub(crate) fn sort_streams(mut self) -> Self {
        let mut vid_streams = Vec::new();
        let mut aud_streams = Vec::new();
        let mut sub_streams = Vec::new();
        let mut und_streams = Vec::new();

        for stream in self.streams {
            match stream.media_type {
                MediaType::Video => vid_streams.push(stream),
                MediaType::Audio => aud_streams.push(stream),
                MediaType::Subtitles => sub_streams.push(stream),
                MediaType::Undefined => und_streams.push(stream),
            }
        }

        vid_streams.sort_by_key(|s| {
            let pixels = s.resolution.map_or(0, |(w, h)| w * h);
            let bandwidth = s.bandwidth.unwrap_or_default();
            Reverse((pixels, bandwidth))
        });

        aud_streams.sort_by_key(|s| {
            let channels = (s.channels.unwrap_or_default() * 10.0) as u32;
            let bandwidth = s.bandwidth.unwrap_or_default();
            Reverse((channels, bandwidth))
        });

        self.streams = vid_streams
            .into_iter()
            .chain(aud_streams)
            .chain(sub_streams)
            .chain(und_streams)
            .collect();

        self
    }

    pub(crate) fn select_streams(
        mut self,
        select_filters: &SelectFilters,
        select_type: SelectType,
    ) -> Result<Self> {
        let selected = StreamSelector::new(&self.streams).select(select_filters, select_type)?;
        self.streams = self
            .streams
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| if selected.contains(&i) { Some(s) } else { None })
            .collect();
        Ok(self)
    }

    pub async fn metadata(&self, config: &DownloadConfig) -> Result<Vec<StreamMetadata>> {
        let mut metadata = Vec::with_capacity(self.streams.len());

        for (i, stream) in self.streams.iter().enumerate() {
            let mut default_kid = stream.default_kid();
            let mut pssh = HashSet::new();

            if let Some(bytes) = stream.fetch_init(config).await? {
                if let Some(kid) = TencBox::from_init(&bytes)?.map(|x| x.default_kid_hex())
                    && (default_kid.is_none() || kid != "00000000000000000000000000000000")
                {
                    default_kid = Some(kid);
                }

                for data in PsshBox::from_init(&bytes)?.boxes {
                    let _ = pssh.insert(data.as_base64());
                }
            }

            metadata.push(StreamMetadata {
                bandwidth: stream.bandwidth,
                channels: stream.channels,
                codecs: stream.codecs.clone(),
                default_kid,
                encryption_type: stream
                    .segments
                    .first()
                    .and_then(|s| s.key.as_ref().map(|k| k.method.clone()))
                    .unwrap_or_default(),
                frame_rate: stream.frame_rate,
                index: i + 1,
                language: stream.language.clone(),
                media_type: stream.media_type.clone(),
                playlist_type: stream.playlist_type.clone(),
                pssh,
                resolution: stream.resolution,
            });
        }

        Ok(metadata)
    }
}
