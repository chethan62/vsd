use crate::{
    core::{fetch, mux::Muxer, sub, vid},
    error::Result,
    playlist::{ClipRange, MasterPlaylist, MediaPlaylist, MediaType},
    progress::Progress,
    select::{SelectFilters, SelectType},
    utils,
};
use colored::Colorize;
use log::{info, warn};
use reqwest::{Client, Url};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Arc,
};
use tokio_util::sync::CancellationToken;
use vsd_mp4::{boxes::TencBox, pssh::PsshBox};

#[derive(Clone, Debug)]
pub struct PlaylistDownloadConfig {
    pub client: Client,
    pub decrypt: bool,
    pub directory: Option<PathBuf>,
    pub keys: HashMap<String, String>,
    pub merge: bool,
    pub query: Arc<Vec<(String, String)>>,
    pub resume: bool,
    pub retries: u8,
    pub threads: u8,
}

pub struct PlaylistDownloader {
    base_url: Option<Url>,
    clip: Option<ClipRange>,
    config: PlaylistDownloadConfig,
    output: Option<PathBuf>,
    select_options: SelectFilters,
    select_type: SelectType,
    subs_codec: String,
}

impl PlaylistDownloader {
    pub fn new(client: &Client) -> Self {
        Self {
            base_url: None,
            clip: None,
            output: None,
            select_options: SelectFilters::new("v=best:s=en"),
            select_type: SelectType::None,
            subs_codec: "copy".to_owned(),
            config: PlaylistDownloadConfig {
                client: client.clone(),
                decrypt: true,
                directory: None,
                keys: HashMap::new(),
                merge: true,
                query: Arc::new(Vec::new()),
                resume: true,
                retries: 10,
                threads: 5,
            },
        }
    }

    pub fn base_url(mut self, base_url: impl Into<Url>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn clip(mut self, clip: &str) -> Result<Self> {
        self.clip = Some(ClipRange::new(clip)?);
        Ok(self)
    }

    pub fn decrypt(mut self, decrypt: bool) -> Self {
        self.config.decrypt = decrypt;
        self
    }

    pub fn directory(mut self, directory: impl Into<PathBuf>) -> Result<Self> {
        let directory = directory.into();

        if !directory.exists() {
            fs::create_dir_all(&directory)?;
        }

        self.config.directory = Some(directory);
        Ok(self)
    }

    pub fn interactive(mut self, raw: bool) -> Self {
        if raw {
            self.select_type = SelectType::Raw;
        } else {
            self.select_type = SelectType::Modern;
        }
        self
    }

    pub fn keys(mut self, keys: HashMap<String, String>) -> Self {
        self.config.keys = keys;
        self
    }

    pub fn merge(mut self, merge: bool) -> Self {
        self.config.merge = merge;
        self
    }

    pub fn output(mut self, output: impl Into<PathBuf>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn query(mut self, query: &str) -> Self {
        if query.is_empty() {
            return self;
        }
        self.config.query = Arc::new(
            query
                .trim_start_matches('?')
                .split('&')
                .filter_map(|x| {
                    if let Some((key, value)) = x.split_once('=') {
                        Some((key.to_owned(), value.to_owned()))
                    } else {
                        None
                    }
                })
                .collect(),
        );
        self
    }

    pub fn resume(mut self, resume: bool) -> Self {
        self.config.resume = resume;
        self
    }

    pub fn retries(mut self, retries: u8) -> Self {
        self.config.retries = retries;
        self
    }

    pub fn select_streams(mut self, select_streams: &str) -> Self {
        self.select_options = SelectFilters::new(select_streams);
        self
    }

    pub fn subs_codec(mut self, subs_codec: impl Into<String>) -> Self {
        self.subs_codec = subs_codec.into();
        self
    }

    pub fn threads(mut self, threads: u8) -> Self {
        self.config.threads = threads;
        self
    }

    pub fn config(&self) -> PlaylistDownloadConfig {
        self.config.clone()
    }

    pub async fn parse(&self, uri: &str, partial_parse: bool) -> Result<MasterPlaylist> {
        let fp = fetch::playlist(&self.config, &self.base_url, uri).await?;
        let mut mp = if partial_parse {
            fp.parse(
                &self.config,
                self.select_options.clone(),
                self.select_type.clone(),
                true,
            )
            .await?
        } else {
            fp.parse(
                &self.config,
                self.select_options.clone(),
                SelectType::None,
                false,
            )
            .await?
        };

        if let Some(clip) = &self.clip {
            mp.clip_streams(clip);
        }

        Ok(mp)
    }

    pub(crate) async fn parse_and_list(self, uri: &str) -> Result<()> {
        let fp = fetch::playlist(&self.config, &self.base_url, uri).await?;
        fp.parse_and_list()?;
        Ok(())
    }

    pub(crate) async fn download(self, uri: &str) -> Result<()> {
        let mp = self.parse(uri, true).await?;
        let streams = mp.streams;

        if !self.config.decrypt {
            dump_pssh_info(&self.config, &streams).await?;
        }

        let token = CancellationToken::new();
        let ctrlc_token = token.clone();

        let ctrlc_handle = tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() && !ctrlc_token.is_cancelled() {
                warn!("Aborting download due to Ctrl+C.");
                ctrlc_token.cancel();
            }

            if tokio::signal::ctrl_c().await.is_ok() {
                warn!("Force exiting due to Ctrl+C.");
                std::process::exit(1);
            }
        });

        let mut muxer = Muxer::new();
        let total = streams.len();

        for (i, stream) in streams.iter().enumerate() {
            info!(
                "DownLd [{}] {}",
                stream.media_type.to_string().green(),
                stream.display().cyan(),
            );

            if stream.segments.is_empty() {
                warn!("Stream skipped because no segments were found.");
                continue;
            }

            let progress =
                Progress::new(&format!("{}/{}", i + 1, total), stream.segments.len(), None);

            if stream.media_type == MediaType::Subtitles {
                muxer.push(sub::download(&self.config, progress, &token, stream).await?);
            } else {
                muxer.push(vid::download(&self.config, progress, &token, stream).await?);
            }
        }

        ctrlc_handle.abort();

        if let Some(output) = &self.output
            && muxer.should_mux(&self.config)
        {
            let Some(ffmpeg) = utils::find_ffmpeg() else {
                bail!("ffmpeg couldn't be located, it's required to continue further.");
            };
            muxer.mux(&ffmpeg, output, &self.subs_codec).await?;
            muxer.clean(self.config.directory.as_deref()).await?;
        }

        Ok(())
    }
}

pub async fn dump_pssh_info(
    config: &PlaylistDownloadConfig,
    streams: &[MediaPlaylist],
) -> Result<()> {
    let mut default_kids = HashSet::new();
    let mut init_segments = Vec::new();

    for stream in streams {
        let Some(bytes) = stream.fetch_init(config).await? else {
            continue;
        };

        if let Some(key_id) = TencBox::from_init(&bytes)?.and_then(|x| {
            let key_id = x.default_kid_hex();
            if key_id == "00000000000000000000000000000000" {
                stream.default_kid()
            } else {
                Some(key_id)
            }
        }) {
            default_kids.insert(key_id);
        }

        init_segments.push(bytes);
    }

    let mut boxes = HashSet::new();
    let mut key_ids = HashSet::new();

    for bytes in &init_segments {
        let pssh = PsshBox::from_init(bytes)?;

        for pssh in pssh.boxes {
            let pssh_base64 = pssh.as_base64();
            if !boxes.insert(pssh_base64.clone()) {
                continue;
            }

            info!(
                "DrmPsh [{}] {}",
                pssh.system_id.to_string().magenta(),
                pssh_base64,
            );
            for key_id in &pssh.key_ids {
                if !key_ids.insert(key_id.clone()) {
                    continue;
                }

                info!(
                    "DrmKid [{}] {}{}",
                    pssh.system_id.to_string().magenta(),
                    key_id,
                    if default_kids.contains(key_id) {
                        " (required)".bold().red()
                    } else {
                        "".normal()
                    },
                );
            }
        }
    }

    Ok(())
}
