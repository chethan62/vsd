use crate::error::Result;
use clap::Args;
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::PathBuf,
};
use vsd_mp4::decrypt::CencDecrypter;

/// Decrypt CENC encrypted mp4 files.
///
/// Supports cenc, cens, cbc1 and cbcs protection schemes.
#[derive(Args, Clone, Debug)]
pub struct Decrypt {
    /// Fragmented mp4 file path to decrypt.
    #[arg(required = true)]
    input: PathBuf,

    /// Decryption key in hex format (32 hex characters).
    #[arg(short, long, required = true, value_name = "KEY")]
    key: String,

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
        let output = self.output.unwrap_or(PathBuf::from(format!(
            "{}.dec.{}",
            self.input.with_extension("").to_string_lossy(),
            self.input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("mp4")
        )));
        let mut decrypter = CencDecrypter::new(&self.key)?;

        tokio::task::spawn_blocking(move || {
            decrypter.decrypt_stream(
                &mut BufReader::new(File::open(&self.input)?),
                &mut BufWriter::new(File::create(&output)?),
                self.init.map(fs::read).transpose()?.as_deref(),
            )
        })
        .await??;
        Ok(())
    }
}
