mod dl;
mod enc;
mod fetch;
mod mux;

pub(crate) mod sub;
pub(crate) mod vid;
pub use mux::{Muxer, Stream};

use crate::{
    error::Result,
    options::{Interaction, SelectOptions},
    playlist::{MasterPlaylist, MediaType},
    utils,
};
use reqwest::{Client, Url};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Clone, Debug)]
pub struct DownloadConfig {
    pub client: Client,
    pub directory: Option<PathBuf>,
    pub keys: HashMap<String, String>,
    pub max_retries: u8,
    pub max_threads: u8,
    pub query: Vec<(String, String)>,
    pub skip_decrypt: bool,
    pub skip_merge: bool,
    pub skip_resume: bool,
}

pub struct Downloader {
    config: DownloadConfig,
    base_url: Option<Url>,
    output: Option<PathBuf>,
    subs_codec: String,
    interaction_type: Interaction,
    select_options: SelectOptions,
}

impl Downloader {
    pub fn new(client: &Client) -> Self {
        Self {
            config: DownloadConfig {
                client: client.clone(),
                directory: None,
                keys: HashMap::new(),
                max_retries: 10,
                max_threads: 5,
                query: Vec::new(),
                skip_decrypt: false,
                skip_merge: false,
                skip_resume: false,
            },
            base_url: None,
            output: None,
            subs_codec: "copy".to_owned(),
            interaction_type: Interaction::None,
            select_options: "v=best:s=en".parse().unwrap(),
        }
    }

    pub fn base_url(mut self, base_url: impl Into<Url>) -> Self {
        self.base_url = Some(base_url.into());
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

    pub fn output(mut self, output: impl Into<PathBuf>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn subs_codec(mut self, subs_codec: impl Into<String>) -> Self {
        self.subs_codec = subs_codec.into();
        self
    }

    pub fn interactive(mut self, raw: bool) -> Self {
        if raw {
            self.interaction_type = Interaction::Raw;
        } else {
            self.interaction_type = Interaction::Modern;
        }
        self
    }

    pub fn select_streams(mut self, select_streams: &str) -> Self {
        self.select_options = select_streams.parse().unwrap();
        self
    }

    pub fn query(mut self, query: &str) -> Self {
        if query.is_empty() {
            return self;
        }
        self.config.query = query
            .trim_start_matches('?')
            .split('&')
            .filter_map(|x| {
                if let Some((key, value)) = x.split_once('=') {
                    Some((key.to_owned(), value.to_owned()))
                } else {
                    None
                }
            })
            .collect();
        self
    }

    pub fn keys(mut self, keys: HashMap<String, String>) -> Self {
        self.config.keys = keys;
        self
    }

    pub fn skip_decrypt(mut self, skip_decrypt: bool) -> Self {
        self.config.skip_decrypt = skip_decrypt;
        self
    }

    pub fn skip_merge(mut self, skip_merge: bool) -> Self {
        self.config.skip_merge = skip_merge;
        self
    }

    pub fn no_resume(mut self, no_resume: bool) -> Self {
        self.config.skip_resume = no_resume;
        self
    }

    pub fn max_retries(mut self, max_retries: u8) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    pub fn max_threads(mut self, max_threads: u8) -> Self {
        self.config.max_threads = max_threads;
        self
    }

    pub fn config(&self) -> DownloadConfig {
        self.config.clone()
    }

    pub async fn parse(&self, uri: &str, partial_parse: bool) -> Result<MasterPlaylist> {
        let fp = fetch::playlist(&self.config, &self.base_url, uri).await?;
        let mut mp = if partial_parse {
            fp.parse(
                &self.config,
                self.select_options.clone(),
                self.interaction_type.clone(),
                true,
            )
            .await?
        } else {
            fp.parse(
                &self.config,
                self.select_options.clone(),
                Interaction::None,
                false,
            )
            .await?
        };

        for stream in &mut mp.streams {
            if stream.media_type != MediaType::Subtitles {
                stream.fetch_split(&self.config).await?;
            }
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
        let muxer = dl::download_streams(&self.config, mp.streams).await?;

        if muxer.should_mux(&self.config, &self.output) {
            let Some(ffmpeg) = utils::find_ffmpeg() else {
                bail!("ffmpeg couldn't be located, it's required to continue further.");
            };
            muxer
                .mux(&ffmpeg, self.output.as_ref().unwrap(), &self.subs_codec)
                .await?;
            muxer.clean(self.config.directory.as_deref()).await?;
        }

        Ok(())
    }
}
