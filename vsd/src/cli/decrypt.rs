use crate::error::Result;
use clap::Args;
use log::info;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::PathBuf,
};
use vsd_mp4::decrypt::CencDecryptingProcessor;

/// Decrypt CENC encrypted mp4 files.
///
/// Supports cenc, cens, cbc1 and cbcs protection schemes.
#[derive(Args, Clone, Debug)]
pub struct Decrypt {
    /// Fragmented mp4 file path to decrypt.
    #[arg(required = true)]
    input: PathBuf,

    /// Decryption keys in KID:KEY hex format.
    #[arg(long, required = true, value_name = "KID:KEY;…", value_parser = super::Save::parse_keys)]
    keys: HashMap<String, String>,

    /// Path to a separate init segment containing moov/tenc boxes.
    #[arg(long, value_name = "PATH")]
    init: Option<PathBuf>,

    /// Output file path for the decrypted file.
    ///
    /// Defaults to the input filename with a .dec suffix before the extension.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
}

impl Decrypt {
    pub async fn execute(self) -> Result<()> {
        let mut processor = CencDecryptingProcessor::builder()
            .keys(&self.keys)?
            .build()?;
        let output = self.output.unwrap_or(
            PathBuf::from(format!(
                "{}.dec.{}",
                self.input.with_extension("").to_string_lossy(),
                self.input.extension().and_then(|e| e.to_str()).unwrap_or("mp4")
            ))
        );

        let fragments = tokio::task::spawn_blocking(move || {
            processor.decrypt_stream(
                &mut BufReader::new(File::open(&self.input)?),
                &mut BufWriter::new(File::create(&output)?),
                self.init.map(|x| fs::read(x)).transpose()?.as_deref(),
            )
        })
        .await??;

        info!("Total {} fragments decrypted.", fragments);
        Ok(())
    }
}
