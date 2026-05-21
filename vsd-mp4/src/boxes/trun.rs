use crate::{ParsedBox, Result};

/// Sample data entry within a track fragment run.
#[derive(Debug, Clone)]
pub struct TrunSample {
    /// The length of the sample in timescale units.
    pub sample_duration: Option<u32>,
    /// The size of the sample in bytes.
    pub sample_size: Option<u32>,
    /// The composition time offset of the sample (difference between composition time and decode time), in timescale units.
    pub sample_composition_time_offset: Option<i32>,
}

/// Track Fragment Run Box (trun) - provides details for each sample in a movie fragment.
#[derive(Debug, Clone)]
pub struct TrunBox {
    /// The number of samples being added in this run.
    pub sample_count: u32,
    /// An array containing data for each sample in the run.
    pub sample_data: Vec<TrunSample>,
    /// If specified via flags, this indicates the offset of the first sample's data in bytes.
    pub data_offset: Option<u32>,
}

impl TrunBox {
    /// Parses a `trun` box from a `ParsedBox`.
    pub fn new(box_: &mut ParsedBox) -> Result<Self> {
        let reader = &mut box_.reader;
        let version = box_.version.unwrap();
        let flags = box_.flags.unwrap();

        let sample_count = reader.read_u32()?;
        let mut sample_data = vec![];
        let mut data_offset = None;

        // "data_offset"
        if (flags & 0x000001) != 0 {
            data_offset = Some(reader.read_u32()?);
        }

        // Skip "first_sample_flags" if present.
        if (flags & 0x000004) != 0 {
            reader.skip(4)?;
        }

        for _ in 0..sample_count {
            let mut sample = TrunSample {
                sample_duration: None,
                sample_size: None,
                sample_composition_time_offset: None,
            };

            // Read "sample duration" if present.
            if (flags & 0x000100) != 0 {
                sample.sample_duration = Some(reader.read_u32()?);
            }

            // Read "sample_size" if present.
            if (flags & 0x000200) != 0 {
                sample.sample_size = Some(reader.read_u32()?);
            }

            // Skip "sample_flags" if present.
            if (flags & 0x000400) != 0 {
                reader.skip(4)?;
            }

            // Read "sample_time_offset" if present.
            if (flags & 0x000800) != 0 {
                sample.sample_composition_time_offset = Some(if version == 0 {
                    reader.read_u32()? as i32
                } else {
                    reader.read_i32()?
                });
            }

            sample_data.push(sample);
        }

        Ok(Self {
            sample_count,
            sample_data,
            data_offset,
        })
    }
}
