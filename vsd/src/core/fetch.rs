use crate::{
    dash, hls,
    options::{Interaction, SelectOptions},
    playlist::{MasterPlaylist, MediaPlaylist, PlaylistType},
    utils,
};
use anyhow::{Result, anyhow, bail};
use base64::Engine;
use colored::Colorize;
use log::{debug, info};
use reqwest::{Client, Url, header};
use std::path::Path;
use tokio::fs;

pub struct FetchedPlaylist {
    url: Url,
    data: Vec<u8>,
    playlist_type: Option<PlaylistType>,
}

impl FetchedPlaylist {
    pub async fn new(
        client: &Client,
        base_url: Option<&Url>,
        query: &[(String, String)],
        input: &str,
    ) -> Result<Self> {
        let path = Path::new(input);
        let mut typ = None;

        if path.exists() {
            if base_url.is_none() {
                bail!("--baseurl flag is required for local playlist file.");
            }

            match path.extension() {
                Some(ext) if ext == "m3u" || ext == "m3u8" => typ = Some(PlaylistType::Hls),
                Some(ext) if ext == "mpd" => typ = Some(PlaylistType::Dash),
                _ => (),
            }

            Ok(Self {
                url: base_url.unwrap().clone(),
                data: fs::read(path).await?,
                playlist_type: typ,
            })
        } else if let Ok(input) = input.parse::<Url>() {
            debug!("Fetching {} (master-playlist)", input);
            let response = client.get(input).query(query).send().await?;

            if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
                match content_type.as_bytes() {
                    b"application/dash+xml" | b"video/vnd.mpeg.dash.mpd" => {
                        typ = Some(PlaylistType::Dash)
                    }
                    b"application/x-mpegurl" | b"application/vnd.apple.mpegurl" => {
                        typ = Some(PlaylistType::Hls)
                    }
                    _ => (),
                }
            }

            Ok(Self {
                url: response.url().to_owned(),
                data: utils::fetch_bytes(response).await?,
                playlist_type: typ,
            })
        } else {
            bail!("Unable to determine the input playlist type.");
        }
    }

    fn playlist_type(&self) -> Result<PlaylistType> {
        if let Some(typ) = &self.playlist_type {
            return Ok(typ.to_owned());
        }
        if self.data.windows(7).any(|w| w == b"#EXTM3U") {
            return Ok(PlaylistType::Hls);
        }
        if self.data.windows(4).any(|w| w == b"<MPD") {
            return Ok(PlaylistType::Dash);
        }
        bail!("Unable to determine the input playlist type.");
    }

    pub fn list_streams(&self) -> Result<()> {
        match self.playlist_type()? {
            PlaylistType::Dash => {
                let xml = String::from_utf8_lossy(&self.data);
                let mpd = dash_mpd::parse(&xml)
                    .map_err(|e| anyhow!("Failed to parse dash playlist: {e}"))?;
                dash::parse_as_master(&self.url, &mpd)
                    .sort_streams()
                    .list_streams();
            }
            PlaylistType::Hls => match m3u8_rs::parse_playlist_res(&self.data)
                .map_err(|e| anyhow!("Failed to parse hls playlist: {e}"))?
            {
                m3u8_rs::Playlist::MasterPlaylist(m3u8) => hls::parse_as_master(&self.url, &m3u8)
                    .sort_streams()
                    .list_streams(),
                m3u8_rs::Playlist::MediaPlaylist(_) => {
                    info!("------ {} ------", "Undefined Streams".cyan());
                    info!(" 1) {}", self.url);
                }
            },
        }
        Ok(())
    }

    pub async fn as_master_playlist(
        &self,
        client: &Client,
        query: &[(String, String)],
        mut select_opts: SelectOptions,
        interaction: Interaction,
        parse_everything: bool,
    ) -> Result<MasterPlaylist> {
        match self.playlist_type()? {
            PlaylistType::Dash => {
                let xml = String::from_utf8_lossy(&self.data);
                let mpd = dash_mpd::parse(&xml)
                    .map_err(|e| anyhow!("Failed to parse dash playlist: {e}"))?;

                let mut playlist = dash::parse_as_master(&self.url, &mpd).sort_streams();

                if !parse_everything {
                    playlist = playlist.select_streams(&mut select_opts, interaction)?;
                }

                for stream in &mut playlist.streams {
                    dash::push_segments(client, &self.url, query, &mpd, stream).await?;
                }

                Ok(playlist)
            }
            PlaylistType::Hls => match m3u8_rs::parse_playlist_res(&self.data)
                .map_err(|e| anyhow!("Failed to parse hls playlist: {e}"))?
            {
                m3u8_rs::Playlist::MasterPlaylist(m3u8) => {
                    let mut playlist = hls::parse_as_master(&self.url, &m3u8).sort_streams();

                    if !parse_everything {
                        playlist = playlist.select_streams(&mut select_opts, interaction)?;
                    }

                    for stream in &mut playlist.streams {
                        let data;
                        if let Some(bs) = stream
                            .uri
                            .strip_prefix("data:application/x-mpegurl;base64,")
                        {
                            data = base64::engine::general_purpose::STANDARD.decode(bs)?;
                            stream.uri = self.url.to_string();
                        } else {
                            stream.uri = self.url.join(&stream.uri)?.to_string();
                            debug!("Fetching {} (media-playlist)", stream.uri);
                            let response = client.get(&stream.uri).query(query).send().await?;
                            data = utils::fetch_bytes(response).await?;
                        }

                        let media_playlist = m3u8_rs::parse_media_playlist_res(&data)
                            .map_err(|e| anyhow!("Failed to parse HLS playlist: {e}"))?;
                        hls::push_segments(stream, media_playlist);
                    }

                    Ok(playlist)
                }
                m3u8_rs::Playlist::MediaPlaylist(playlist) => {
                    let mut stream = MediaPlaylist {
                        id: utils::gen_id(self.url.as_str(), ""),
                        playlist_type: PlaylistType::Hls,
                        uri: self.url.to_string(),
                        ..Default::default()
                    };
                    hls::push_segments(&mut stream, playlist);
                    Ok(MasterPlaylist {
                        playlist_type: PlaylistType::Hls,
                        streams: vec![stream],
                        uri: self.url.as_str().to_owned(),
                    })
                }
            },
        }
    }
}
