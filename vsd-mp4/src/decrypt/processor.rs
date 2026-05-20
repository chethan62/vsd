use crate::{
    Mp4Parser,
    boxes::{SchmBox, SencBox, TencBox, TrunBox},
    data,
    decrypt::{decrypter::Decrypter, stream::BoxHeader},
    error::{Error, Result},
    parser,
};
use std::io::{Read, Write};

struct TrackEncInfo {
    scheme_type: u32,
    tenc: TencBox,
}

pub struct CencDecrypter {
    key: [u8; 16],
    track: Option<TrackEncInfo>,
}

impl CencDecrypter {
    pub fn new(key: &str) -> Result<Self> {
        Ok(Self {
            key: hex::decode(key.to_ascii_lowercase().replace('-', ""))?
                .try_into()
                .map_err(|v: Vec<u8>| Error::InvalidKeySize(v.len()))?,
            track: None,
        })
    }

    pub fn with_init(key: &str, init: &[u8]) -> Result<Self> {
        let mut decrypter = Self::new(key)?;
        decrypter.track = Some(Self::parse_track(init)?);
        Ok(decrypter)
    }

    pub fn decrypt(&self, input: Vec<u8>, init: Option<&[u8]>) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(input);
        }

        let owned;
        let track = if let Some(init) = init {
            owned = Self::parse_track(init)?;
            &owned
        } else if let Some(ref cached) = self.track {
            cached
        } else {
            owned = Self::parse_track(&input)?;
            &owned
        };

        Self::decrypt_fragment(&self.key, track, input)
    }

    pub fn decrypt_stream<R: Read, W: Write>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        init: Option<&[u8]>,
    ) -> Result<u64> {
        let (init_data, first_moof_header) = match init {
            Some(data) => (data.to_vec(), None),
            None => {
                let (data, moof) = BoxHeader::read_init(reader)?;
                if moof.is_none() {
                    return Err(Error::InvalidMp4(
                        "No moof box found — input does not appear to be a fragmented mp4".into(),
                    ));
                }
                (data, moof)
            }
        };

        self.track = Some(Self::parse_track(&init_data)?);

        if init.is_none() {
            writer.write_all(&init_data)?;
        }

        let mut pending = first_moof_header;
        let mut fragments: u64 = 0;

        loop {
            let header = match pending.take() {
                Some(h) => h,
                None => match BoxHeader::read_header(reader)? {
                    Some(h) => h,
                    None => break,
                },
            };

            if &header.box_type == b"moof" {
                let moof_data = header.read_data(reader)?;
                let mut fragment = moof_data;

                loop {
                    let Some(next) = BoxHeader::read_header(reader)? else {
                        break;
                    };

                    let next_data = next.read_data(reader)?;
                    fragment.extend_from_slice(&next_data);

                    if &next.box_type == b"mdat" {
                        break;
                    }
                }

                let decrypted = self.decrypt(fragment, None)?;
                writer.write_all(&decrypted)?;

                fragments += 1;
            } else {
                let box_data = header.read_data(reader)?;
                writer.write_all(&box_data)?;
            }
        }

        writer.flush()?;

        Ok(fragments)
    }

    fn parse_track(init_data: &[u8]) -> Result<TrackEncInfo> {
        let current_schm = data!(0u32);
        let current_tenc = data!();
        let result = data!();

        let _ = Mp4Parser::new()
            .base_box("moov", parser::children)
            .base_box("trak", {
                let current_schm = current_schm.clone();
                let current_tenc = current_tenc.clone();
                let result = result.clone();
                move |box_| {
                    *current_schm.borrow_mut() = 0;
                    *current_tenc.borrow_mut() = None;

                    parser::children(box_)?;

                    if result.borrow().is_none() {
                        if let Some(tenc) = current_tenc.borrow_mut().take() {
                            let scheme = *current_schm.borrow();
                            *result.borrow_mut() = Some(TrackEncInfo {
                                scheme_type: scheme,
                                tenc,
                            });
                        }
                    }
                    Ok(())
                }
            })
            .full_box("tkhd", |_| Ok(()))
            .base_box("mdia", parser::children)
            .base_box("minf", parser::children)
            .base_box("stbl", parser::children)
            .full_box("stsd", parser::sample_description)
            .base_box("encv", parser::visual_sample_entry)
            .base_box("enca", parser::audio_sample_entry)
            .base_box("sinf", parser::children)
            .full_box("schm", {
                let current_schm = current_schm.clone();
                move |mut box_| {
                    *current_schm.borrow_mut() = SchmBox::new(&mut box_)?.scheme_type;
                    Ok(())
                }
            })
            .base_box("schi", parser::children)
            .full_box("tenc", {
                let current_tenc = current_tenc.clone();
                move |mut box_| {
                    *current_tenc.borrow_mut() = Some(TencBox::new(&mut box_)?);
                    Ok(())
                }
            })
            .parse(init_data, true, true);

        result
            .borrow_mut()
            .take()
            .ok_or_else(|| Error::InvalidMp4("No encrypted track found (no tenc box)".into()))
    }

    fn decrypt_fragment(
        key: &[u8; 16],
        track: &TrackEncInfo,
        mut input_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        struct FragmentInfo {
            trun: TrunBox,
            senc: SencBox,
        }

        let moof_start = data!(0u64);
        let fragments = data!(Vec::new());

        let current_frag_trun = data!();
        let current_frag_senc = data!();

        let iv_size = track.tenc.per_sample_iv_size;
        let constant_iv = track.tenc.constant_iv.clone();

        let _ = Mp4Parser::new()
            .base_box("moof", {
                let moof_start = moof_start.clone();
                move |box_| {
                    *moof_start.borrow_mut() = box_.start;
                    parser::children(box_)
                }
            })
            .base_box("traf", {
                let current_frag_trun = current_frag_trun.clone();
                let current_frag_senc = current_frag_senc.clone();
                let fragments = fragments.clone();
                move |box_| {
                    *current_frag_trun.borrow_mut() = None;
                    *current_frag_senc.borrow_mut() = None;

                    parser::children(box_)?;

                    let trun = current_frag_trun.borrow_mut().take();
                    let senc = current_frag_senc.borrow_mut().take();

                    if let (Some(trun), Some(senc)) = (trun, senc) {
                        fragments.borrow_mut().push(FragmentInfo { trun, senc });
                    }
                    Ok(())
                }
            })
            .full_box("tfhd", |_| Ok(()))
            .full_box("tfdt", |_| Ok(()))
            .full_box("trun", {
                let current_frag_trun = current_frag_trun.clone();
                move |mut box_| {
                    *current_frag_trun.borrow_mut() = Some(TrunBox::new(&mut box_)?);
                    Ok(())
                }
            })
            .full_box("senc", {
                let current_frag_senc = current_frag_senc.clone();
                let constant_iv = constant_iv.clone();
                move |mut box_| {
                    *current_frag_senc.borrow_mut() =
                        Some(SencBox::new(&mut box_, iv_size, constant_iv.as_deref())?);
                    Ok(())
                }
            })
            .parse(&input_data, true, true);

        let frags = fragments.borrow();
        if frags.is_empty() {
            return Ok(input_data);
        }

        let moof_start_val = *moof_start.borrow();

        let mut decrypter = Decrypter::new(
            track.scheme_type,
            key,
            track.tenc.crypt_byte_block,
            track.tenc.skip_byte_block,
        );

        for frag in frags.iter() {
            let data_start = {
                let offset = frag.trun.data_offset.unwrap_or_default() as i64;
                (moof_start_val as i64 + offset) as usize
            };

            let mut offset = data_start;
            let output_len = input_data.len();

            for (trun_sample, senc_sample) in
                frag.trun.sample_data.iter().zip(frag.senc.samples.iter())
            {
                let size = trun_sample.sample_size.unwrap_or_default() as usize;
                if size == 0 {
                    continue;
                }

                let end = offset + size;
                if end > output_len {
                    break;
                }

                decrypter.decrypt_sample_inplace(&mut input_data[offset..end], senc_sample);
                offset = end;
            }
        }

        Ok(input_data)
    }
}
