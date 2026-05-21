use crate::{ParsedBox, Result};

/// Media Header Box (mdhd) - declares media-independent metadata and information.
///
/// The Media Header Box contains the overall information and media-independent metadata
/// about the media within a track.
#[derive(Debug, Clone)]
pub struct MdhdBox {
    /// The time-scale for this media. This is the number of time units that pass in one second.
    pub timescale: u32,
    /// The ISO 639-2/T 3-character language code for this media (e.g., "und", "eng").
    pub language: String,
}

impl MdhdBox {
    /// Parses a `mdhd` box from a `ParsedBox`.
    ///
    /// This method parses the creation time, modification time, timescale, duration,
    /// and language from the box payload according to the box version (0 or 1).
    pub fn new(box_: &mut ParsedBox) -> Result<Self> {
        let reader = &mut box_.reader;
        let version = box_.version.unwrap();

        if version == 1 {
            reader.skip(8)?;
            reader.skip(8)?;
        } else {
            reader.skip(4)?;
            reader.skip(4)?;
        }

        let timescale = reader.read_u32()?;

        reader.skip(4)?;

        let language = reader.read_u16()?;

        let language_string = String::from_utf16(&[
            (language >> 10) + 0x60,
            ((language & 0x03c0) >> 5) + 0x60,
            (language & 0x1f) + 0x60,
        ])?;

        Ok(Self {
            timescale,
            language: language_string,
        })
    }
}
