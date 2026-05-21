use crate::{ParsedBox, Result};

/// Scheme Type Box (schm) - identifies the protection scheme.
///
/// The Scheme Type Box is used to identify the protection scheme applied to the track.
/// Key scheme types include:
/// - `cenc` (0x63656E63) - AES-CTR full sample encryption
/// - `cens` (0x63656E73) - AES-CTR subsample pattern encryption
/// - `cbc1` (0x63626331) - AES-CBC full sample encryption
/// - `cbcs` (0x63626373) - AES-CBC pattern encryption
#[derive(Debug, Clone)]
pub struct SchmBox {
    /// The scheme type as a 4-byte code (e.g., 'cenc', 'cbcs').
    pub scheme_type: u32,
    /// The version of the scheme.
    pub scheme_version: u32,
    /// An optional URI providing details on the protection scheme.
    pub scheme_uri: Option<String>,
}

impl SchmBox {
    /// Parses a `schm` box from a `ParsedBox`.
    ///
    /// This method parses the scheme type, version, and optional URI if the
    /// appropriate flag is set.
    pub fn new(box_: &mut ParsedBox) -> Result<Self> {
        let reader = &mut box_.reader;
        let flags = box_.flags.unwrap_or(0);

        let scheme_type = reader.read_u32()?;
        let scheme_version = reader.read_u32()?;

        // If flags indicate scheme_uri is present (flag 0x000001)
        let scheme_uri = if flags & 0x000001 != 0 {
            let remaining = (reader.get_length() - reader.get_position()) as usize;
            if remaining > 0 {
                let bytes = reader.read_bytes_u8(remaining)?;
                let s = String::from_utf8_lossy(&bytes);
                Some(s.trim_end_matches('\0').to_string())
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            scheme_type,
            scheme_version,
            scheme_uri,
        })
    }
}
