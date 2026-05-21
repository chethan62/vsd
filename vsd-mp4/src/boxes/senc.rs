use crate::{ParsedBox, Result};

/// A subsample encryption entry, dividing a sample into clear (unencrypted) and encrypted bytes.
#[derive(Debug, Clone)]
pub struct SencSubsample {
    /// Number of clear (unencrypted) bytes in this subsample.
    pub bytes_of_clear_data: u16,
    /// Number of encrypted bytes in this subsample.
    pub bytes_of_encrypted_data: u32,
}

/// Encryption parameters for a single media sample.
#[derive(Debug, Clone)]
pub struct SencSample {
    /// The initialization vector (IV) for this sample. Shorter IVs (e.g. 8 bytes) are zero-padded to 16 bytes.
    pub iv: [u8; 16],
    /// The list of subsample encryption entries if subsample-level encryption is active.
    pub subsamples: Vec<SencSubsample>,
}

/// Sample Encryption Box (senc) - contains initialization vectors and subsample maps for all samples in a movie fragment.
#[derive(Debug, Clone)]
pub struct SencBox {
    /// The flags from the box header (typically indicating whether subsamples are present).
    pub flags: u32,
    /// Per-sample encryption parameters.
    pub samples: Vec<SencSample>,
}

impl SencBox {
    /// Parses a `senc` box from a `ParsedBox`.
    ///
    /// # Arguments
    /// * `box_` - The parsed box to read from.
    /// * `iv_size` - The IV size in bytes (from `tenc` default or per-sample IV size).
    /// * `constant_iv` - An optional constant IV to use if `iv_size` is zero.
    pub fn new(box_: &mut ParsedBox, iv_size: u8, constant_iv: Option<&[u8; 16]>) -> Result<Self> {
        let reader = &mut box_.reader;
        let flags = box_.flags.unwrap_or(0);

        let sample_count = reader.read_u32()?;
        let has_subsamples = flags & 0x02 != 0;

        let mut samples = Vec::with_capacity(sample_count as usize);

        for _ in 0..sample_count {
            let iv = if iv_size > 0 && iv_size <= 16 {
                let bytes = reader.read_bytes_u8(iv_size as usize)?;
                let mut iv = [0u8; 16];
                iv[..bytes.len()].copy_from_slice(&bytes);
                iv
            } else if let Some(civ) = constant_iv {
                *civ
            } else {
                [0u8; 16]
            };

            let subsamples = if has_subsamples {
                let subsample_count = reader.read_u16()?;
                let mut subs = Vec::with_capacity(subsample_count as usize);
                for _ in 0..subsample_count {
                    let bytes_of_clear_data = reader.read_u16()?;
                    let bytes_of_encrypted_data = reader.read_u32()?;
                    subs.push(SencSubsample {
                        bytes_of_clear_data,
                        bytes_of_encrypted_data,
                    });
                }
                subs
            } else {
                Vec::new()
            };

            samples.push(SencSample { iv, subsamples });
        }

        Ok(Self { flags, samples })
    }
}
