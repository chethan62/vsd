use crate::{
    downloader::{SKIP_DECRYPT, SKIP_MERGE},
    playlist::{MediaPlaylist, MediaType},
};
use anyhow::{Result, bail};
use colored::Colorize;
use log::{debug, info, warn};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::Ordering,
};
use tokio::{fs, process::Command};

pub struct Streams(pub Vec<Stream>);

pub struct Stream {
    pub language: Option<String>,
    pub media_type: MediaType,
    pub path: PathBuf,
}

impl Streams {
    pub async fn mux(&self, ffmpeg: &Path, output: &Path, subs_codec: &str) -> Result<()> {
        let temp_files = [MediaType::Video, MediaType::Audio, MediaType::Subtitles]
            .iter()
            .flat_map(|mt| self.0.iter().filter(move |x| x.media_type == *mt))
            .collect::<Vec<_>>();

        if temp_files.is_empty() {
            bail!("No streams available for muxing.");
        }

        let mut args = vec!["-hide_banner".to_owned(), "-y".to_owned()];

        for temp_file in &temp_files {
            args.extend_from_slice(&["-i".to_owned(), temp_file.path.to_string_lossy().into()]);
        }

        if temp_files.len() > 1 {
            for i in 0..temp_files.len() {
                args.extend_from_slice(&["-map".to_owned(), i.to_string()]);
            }

            let mut aud_idx = 0u8;
            let mut sub_idx = 0u8;

            for temp_file in &temp_files {
                match temp_file.media_type {
                    MediaType::Audio => {
                        if let Some(language) = &temp_file.language {
                            args.extend_from_slice(&[
                                format!("-metadata:s:a:{aud_idx}"),
                                format!("language={language}"),
                            ]);
                        }
                        aud_idx += 1;
                    }
                    MediaType::Subtitles => {
                        if let Some(language) = &temp_file.language {
                            args.extend_from_slice(&[
                                format!("-metadata:s:s:{sub_idx}"),
                                format!("language={language}"),
                            ]);
                        }
                        sub_idx += 1;
                    }
                    _ => (),
                }
            }

            if aud_idx > 0 {
                args.extend_from_slice(&["-disposition:a:0".to_owned(), "default".to_owned()]);
            }

            if sub_idx > 0 {
                args.extend_from_slice(&["-disposition:s:0".to_owned(), "default".to_owned()]);
            }
        }

        let has_vid = temp_files.iter().any(|x| x.media_type == MediaType::Video);
        let has_aud = temp_files.iter().any(|x| x.media_type == MediaType::Audio);
        let has_sub = temp_files
            .iter()
            .any(|x| x.media_type == MediaType::Subtitles);

        if has_vid {
            args.extend_from_slice(&["-c:v".to_owned(), "copy".to_owned()]);
        }
        if has_aud {
            args.extend_from_slice(&["-c:a".to_owned(), "copy".to_owned()]);
        }
        if has_sub {
            let codec = if subs_codec == "copy" {
                match output.extension().and_then(|e| e.to_str()) {
                    Some("mp4" | "m4v" | "mov") => "mov_text",
                    Some("srt") => "srt",
                    Some("vtt") => "webvtt",
                    Some("ass" | "ssa") => "ass",
                    _ => "copy",
                }
            } else {
                subs_codec
            };
            args.extend_from_slice(&["-c:s".to_owned(), codec.to_owned()]);
        }

        args.push(output.to_string_lossy().into());

        info!(
            "Muxing [{}] ffmpeg {}",
            "exe".cyan(),
            args.iter()
                .map(|x| if x.contains(' ') {
                    format!("\"{x}\"")
                } else {
                    x.to_owned()
                })
                .collect::<Vec<_>>()
                .join(" ")
        );

        let result = Command::new(ffmpeg)
            .args(&args)
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            bail!(
                "ffmpeg exited with code {}: {}",
                result.status.code().unwrap_or(1),
                stderr.lines().last().unwrap_or("unknown error.")
            );
        }

        Ok(())
    }

    pub async fn clean(&self, directory: Option<&Path>) -> Result<()> {
        for stream in &self.0 {
            if stream.path.exists() {
                debug!("Deleting '{}' file.", stream.path.to_string_lossy());
                fs::remove_file(&stream.path).await?;
            }
        }
        if let Some(directory) = directory
            && directory.read_dir()?.next().is_none()
        {
            debug!("Deleting '{}' directory.", directory.to_string_lossy());
            fs::remove_dir(directory).await?;
        }
        Ok(())
    }
}

pub fn should_mux(streams: &[MediaPlaylist], output: Option<&PathBuf>) -> bool {
    if output.is_none() {
        return false;
    }
    if SKIP_DECRYPT.load(Ordering::SeqCst) {
        warn!("--output is ignored when --no-decrypt is used.");
        return false;
    }
    if SKIP_MERGE.load(Ordering::SeqCst) {
        warn!("--output is ignored when --no-merge is used.");
        return false;
    }
    if streams
        .iter()
        .filter(|x| x.media_type == MediaType::Video)
        .count()
        > 1
    {
        warn!("--output is ignored when multiple vid streams are selected.");
        return false;
    }
    true
}
