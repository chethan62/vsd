use crate::error::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use tokio::fs;
use vsd_mp4::sub::{StppSubsParser, WvttSubsParser};

/// Extract subtitles from a fragmented mp4 file.
#[derive(Args, Clone, Debug)]
pub struct Extract {
    /// Fragmented mp4 file path containing either wvtt (webvtt) or stpp (ttml) boxes.
    ///
    /// For multiple segments, use merge sub-command to combine them into a single file first.
    #[arg(required = true)]
    input: PathBuf,

    /// Codec for output subtitles.
    #[arg(short, long, value_enum, default_value_t = Codec::Webvtt)]
    codec: Codec,

    /// Output file path for extracted subtitles.
    ///
    /// The codec is inferred from the file extension (.srt or .vtt).
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum)]
enum Codec {
    Subrip,
    Webvtt,
}

impl Extract {
    pub async fn execute(self) -> Result<()> {
        let data = fs::read(self.input).await?;
        let subtitles;

        if let Ok(vtt) = WvttSubsParser::from_init(&data) {
            subtitles = vtt.parse(&data, None)?;
        } else if let Ok(ttml) = StppSubsParser::from_init(&data) {
            subtitles = ttml.parse(&data)?;
        } else {
            bail!(
                "Unable to determine the subtitle codec because neither wvtt nor stpp boxes were found."
            );
        }

        if let Some(path) = self.output {
            let ext = path
                .extension()
                .and_then(|x| match x.to_str() {
                    Some("srt") => Some(Codec::Subrip),
                    Some("vtt") => Some(Codec::Webvtt),
                    _ => None,
                })
                .unwrap_or(self.codec);
            fs::write(
                &path,
                match ext {
                    Codec::Subrip => subtitles.as_srt(),
                    Codec::Webvtt => subtitles.as_vtt(),
                },
            )
            .await?;
        } else {
            print!(
                "{}",
                match &self.codec {
                    Codec::Subrip => subtitles.as_srt(),
                    Codec::Webvtt => subtitles.as_vtt(),
                }
            );
        }

        Ok(())
    }
}
