use crate::{
    Mp4Parser,
    boxes::{SchmBox, SencBox, TencBox, TrunBox},
    data,
    decrypt::{cipher::CencProcessor, reader::BoxHeader},
    error::{Error, Result},
    parser,
};
use std::io::{Read, Write};

#[derive(Clone, Copy, Default)]
struct TrackEncInfo {
    scheme_type: u32,
    per_sample_iv_size: u8,
    constant_iv: Option<[u8; 16]>,
    crypt_byte_block: u8,
    skip_byte_block: u8,
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

    pub fn decrypt(&self, mut input: Vec<u8>, init: Option<&[u8]>) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(input);
        }

        let track;
        let track_ref = if let Some(init) = init {
            track = Self::parse_track(init)?;
            &track
        } else if let Some(cached) = &self.track {
            cached
        } else {
            track = Self::parse_track(&input)?;
            &track
        };

        Self::decrypt_fragment(&self.key, track_ref, &mut input)?;
        Ok(input)
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
                        "no moof box found, input does not appear to be a fragmented mp4.".into(),
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

    fn parse_track(init: &[u8]) -> Result<TrackEncInfo> {
        let track = data!(Option::<TrackEncInfo>::None);

        Mp4Parser::new()
            .base_box("enca", parser::audio_sample_entry)
            .base_box("encv", parser::visual_sample_entry)
            .base_box("mdia", parser::children)
            .base_box("minf", parser::children)
            .base_box("moov", parser::children)
            .base_box("schi", parser::children)
            .base_box("sinf", parser::children)
            .base_box("stbl", parser::children)
            .full_box("stsd", parser::sample_description)
            .full_box("schm", {
                let track = track.clone();
                move |mut box_| {
                    if let Some(t) = &mut *track.borrow_mut() {
                        t.scheme_type = SchmBox::new(&mut box_)?.scheme_type;
                    }
                    Ok(())
                }
            })
            .full_box("tenc", {
                let track = track.clone();
                move |mut box_| {
                    if let Some(t) = &mut *track.borrow_mut() {
                        let tenc = TencBox::new(&mut box_)?;
                        t.per_sample_iv_size = tenc.per_sample_iv_size;
                        t.constant_iv = tenc.constant_iv;
                        t.crypt_byte_block = tenc.crypt_byte_block;
                        t.skip_byte_block = tenc.skip_byte_block;
                    }
                    box_.parser.stop();
                    Ok(())
                }
            })
            .base_box("trak", {
                let track = track.clone();
                move |box_| {
                    *track.borrow_mut() = Some(TrackEncInfo::default());
                    parser::children(box_)?;
                    let t = track.borrow();

                    if let Some(t) = &*t
                        && !(t.per_sample_iv_size > 0 || t.constant_iv.is_some())
                    {
                        *track.borrow_mut() = None;
                    }
                    Ok(())
                }
            })
            .parse(init, true, true)?;

        track
            .borrow_mut()
            .take()
            .ok_or_else(|| Error::InvalidMp4("No encrypted track found (no tenc box)".into()))
    }

    fn decrypt_fragment(key: &[u8; 16], track: &TrackEncInfo, input: &mut Vec<u8>) -> Result<()> {
        struct FragmentInfo {
            trun: TrunBox,
            senc: SencBox,
        }

        #[derive(Default)]
        struct FragmentParserState {
            moof_start: u64,
            fragments: Vec<FragmentInfo>,
            current_frag_trun: Option<TrunBox>,
            current_frag_senc: Option<SencBox>,
        }

        let state = data!(FragmentParserState::default());

        let iv_size = track.per_sample_iv_size;
        let constant_iv = track.constant_iv;

        let _ = Mp4Parser::new()
            .base_box("moof", {
                let state = state.clone();
                move |box_| {
                    state.borrow_mut().moof_start = box_.start;
                    parser::children(box_)
                }
            })
            .base_box("traf", {
                let state = state.clone();
                move |box_| {
                    {
                        let mut s = state.borrow_mut();
                        s.current_frag_trun = None;
                        s.current_frag_senc = None;
                    }

                    parser::children(box_)?;

                    let mut s = state.borrow_mut();
                    let trun = s.current_frag_trun.take();
                    let senc = s.current_frag_senc.take();

                    if let (Some(trun), Some(senc)) = (trun, senc) {
                        s.fragments.push(FragmentInfo { trun, senc });
                    }
                    Ok(())
                }
            })
            .full_box("tfhd", |_| Ok(()))
            .full_box("tfdt", |_| Ok(()))
            .full_box("trun", {
                let state = state.clone();
                move |mut box_| {
                    state.borrow_mut().current_frag_trun = Some(TrunBox::new(&mut box_)?);
                    Ok(())
                }
            })
            .full_box("senc", {
                let state = state.clone();
                let constant_iv = constant_iv;
                move |mut box_| {
                    state.borrow_mut().current_frag_senc =
                        Some(SencBox::new(&mut box_, iv_size, constant_iv.as_ref())?);
                    Ok(())
                }
            })
            .parse(&input, true, true);

        let mut state_borrow = state.borrow_mut();
        let frags = std::mem::take(&mut state_borrow.fragments);
        if frags.is_empty() {
            return Ok(());
        }

        let moof_start_val = state_borrow.moof_start;

        let mut processor = CencProcessor::new(
            key,
            track.crypt_byte_block,
            track.skip_byte_block,
            track.scheme_type,
        );

        for frag in frags.iter() {
            let data_start = {
                let offset = frag.trun.data_offset.unwrap_or_default() as i64;
                (moof_start_val as i64 + offset) as usize
            };

            let mut offset = data_start;
            let output_len = input.len();

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

                processor.decrypt_sample_inplace(&mut input[offset..end], senc_sample);
                offset = end;
            }
        }

        Ok(())
    }
}
