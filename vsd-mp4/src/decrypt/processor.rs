use crate::{
    Mp4Parser,
    boxes::{SchmBox, SencBox, TencBox, TfhdBox, TrunBox},
    data,
    decrypt::{
        decrypter::Decrypter,
        error::{DecryptError, Result},
        stream::BoxHeader,
    },
    parser,
};
use std::collections::HashMap;

#[derive(Default)]
pub struct CencDecryptingProcessorBuilder {
    keys: HashMap<[u8; 16], [u8; 16]>,
}

impl CencDecryptingProcessorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn key(mut self, kid: &str, key: &str) -> Result<Self> {
        self.keys.insert(parse_hex_16(kid)?, parse_hex_16(key)?);
        Ok(self)
    }

    pub fn keys(mut self, keys: &HashMap<String, String>) -> Result<Self> {
        for (kid, key) in keys {
            self.keys.insert(parse_hex_16(kid)?, parse_hex_16(key)?);
        }
        Ok(self)
    }

    pub fn build(self) -> Result<CencDecryptingProcessor> {
        if self.keys.is_empty() {
            return Err(DecryptError::NoKeys);
        }
        Ok(CencDecryptingProcessor {
            keys: self.keys,
            tracks: None,
        })
    }
}

/// Per-track encryption info extracted from an init segment.
#[derive(Clone)]
struct TrackEncInfo {
    scheme_type: u32,
    tenc: TencBox,
}

pub struct CencDecryptingProcessor {
    keys: HashMap<[u8; 16], [u8; 16]>,
    /// Pre-parsed track encryption info.  Populated by [`decrypt_stream`] or
    /// [`decrypt`] so that subsequent calls can skip init parsing.
    tracks: Option<HashMap<u32, TrackEncInfo>>,
}

impl CencDecryptingProcessor {
    pub fn builder() -> CencDecryptingProcessorBuilder {
        CencDecryptingProcessorBuilder::new()
    }

    /// Decrypt a CENC-encrypted fragment.
    ///
    /// When `init_data` is provided it is parsed to obtain track encryption
    /// info.  If tracks have been cached (via a prior [`decrypt_stream`]
    /// call), `init_data` can be `None` and the cached info is used instead.
    pub fn decrypt(&self, input_data: Vec<u8>, init_data: Option<&[u8]>) -> Result<Vec<u8>> {
        if input_data.is_empty() {
            return Ok(input_data);
        }

        let owned_tracks;
        let tracks = if let Some(init) = init_data {
            owned_tracks = parse_tracks(init)?;
            &owned_tracks
        } else if let Some(ref cached) = self.tracks {
            cached
        } else {
            // No explicit init and no cached tracks — try using the input
            // itself (works for single-file fragmented mp4 where moov precedes
            // the fragments).
            owned_tracks = parse_tracks(&input_data)?;
            &owned_tracks
        };

        decrypt_fragment(&self.keys, tracks, input_data)
    }

    /// Return the default KIDs for all encrypted tracks found in the loaded
    /// init segment.  Each entry is `(track_id, kid_hex)`.
    ///
    /// Returns an empty vec if no init segment has been parsed yet.
    pub fn default_kids(&self) -> Vec<(u32, String)> {
        match &self.tracks {
            Some(tracks) => tracks
                .iter()
                .map(|(&tid, info)| (tid, hex::encode(info.tenc.default_kid)))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Check whether a decryption key has been supplied for the given KID
    /// (hex string, case-insensitive).
    pub fn has_key(&self, kid_hex: &str) -> bool {
        if let Ok(kid_bytes) = parse_hex_16(&kid_hex.to_ascii_lowercase().replace('-', "")) {
            self.keys.contains_key(&kid_bytes)
        } else {
            false
        }
    }

    pub fn decrypt_stream<R, W>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        init: Option<&[u8]>,
    ) -> Result<u64>
    where
        R: std::io::Read,
        W: std::io::Write,
    {
        // ----- Init loading -----
        let (init_data, first_moof_header) = match init {
            Some(data) => (data.to_vec(), None),
            None => {
                let (data, moof) = BoxHeader::read_init(reader)?;
                if moof.is_none() {
                    return Err(DecryptError::InvalidFormat(
                        "No moof box found — input does not appear to be a fragmented mp4".into(),
                    ));
                }
                (data, moof)
            }
        };

        self.tracks = Some(parse_tracks(&init_data)?);

        // ----- Key validation -----
        let kids = self.default_kids();
        let missing: Vec<String> = kids
            .iter()
            .filter(|(_, kid)| !self.has_key(kid))
            .map(|(_, kid)| kid.clone())
            .collect();

        if !missing.is_empty() {
            return Err(DecryptError::KeyNotFound(missing.join(", ")));
        }

        // ----- Write init data passthrough -----
        if init.is_none() {
            // Init was read from the stream — write it to the output.
            writer.write_all(&init_data)?;
        }

        // ----- Stream fragments -----
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

                // Read subsequent boxes until (and including) the matching mdat.
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
                // Non-fragment box (e.g. styp, sidx, mfra) — pass through.
                let box_data = header.read_data(reader)?;
                writer.write_all(&box_data)?;
            }
        }

        writer.flush()?;

        Ok(fragments)
    }
}

// ---------------------------------------------------------------------------
// Init-segment parsing — extract per-track encryption info
// ---------------------------------------------------------------------------

fn parse_tracks(init_data: &[u8]) -> Result<HashMap<u32, TrackEncInfo>> {
    let current_track_id = data!(0u32);
    let current_schm = data!(0u32);
    let current_tenc = data!();
    let tracks_map = data!(HashMap::new());

    let _ = Mp4Parser::new()
        .base_box("moov", parser::children)
        .base_box("trak", {
            let current_track_id = current_track_id.clone();
            let current_schm = current_schm.clone();
            let current_tenc = current_tenc.clone();
            let tracks_map = tracks_map.clone();
            move |box_| {
                *current_track_id.borrow_mut() = 0;
                *current_schm.borrow_mut() = 0;
                *current_tenc.borrow_mut() = None;

                parser::children(box_)?;

                if let Some(tenc) = current_tenc.borrow_mut().take() {
                    let tid = *current_track_id.borrow();
                    let scheme = *current_schm.borrow();
                    tracks_map.borrow_mut().insert(
                        tid,
                        TrackEncInfo {
                            scheme_type: scheme,
                            tenc,
                        },
                    );
                }
                Ok(())
            }
        })
        .full_box("tkhd", {
            let current_track_id = current_track_id.clone();
            move |mut box_| {
                let version = box_.version.unwrap_or(0);
                let reader = &mut box_.reader;
                if version >= 1 {
                    reader.skip(16)?;
                } else {
                    reader.skip(8)?;
                }
                *current_track_id.borrow_mut() = reader.read_u32()?;
                Ok(())
            }
        })
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

    let tracks = tracks_map.borrow_mut().drain().collect::<HashMap<_, _>>();

    if tracks.is_empty() {
        return Err(DecryptError::InvalidFormat(
            "No encrypted tracks found (no tenc boxes)".into(),
        ));
    }

    Ok(tracks)
}

// ---------------------------------------------------------------------------
// Fragment decryption — handle multiple traf boxes
// ---------------------------------------------------------------------------

/// Per-track fragment info parsed from a traf box.
struct TrackFragment {
    track_id: u32,
    trun: TrunBox,
    senc: SencBox,
}

fn decrypt_fragment(
    keys: &HashMap<[u8; 16], [u8; 16]>,
    tracks: &HashMap<u32, TrackEncInfo>,
    mut input_data: Vec<u8>,
) -> Result<Vec<u8>> {
    let moof_start = data!(0u64);
    let fragments = data!(Vec::new());

    let current_frag_track_id = data!(0u32);
    let current_frag_trun = data!();
    let current_frag_senc = data!();

    // Owned lookup for the senc closure.
    let iv_info: HashMap<u32, (u8, Option<Vec<u8>>)> = tracks
        .iter()
        .map(|(&tid, info)| {
            (
                tid,
                (info.tenc.per_sample_iv_size, info.tenc.constant_iv.clone()),
            )
        })
        .collect();

    let _ = Mp4Parser::new()
        .base_box("moof", {
            let moof_start = moof_start.clone();
            move |box_| {
                *moof_start.borrow_mut() = box_.start;
                parser::children(box_)
            }
        })
        .base_box("traf", {
            let current_frag_track_id = current_frag_track_id.clone();
            let current_frag_trun = current_frag_trun.clone();
            let current_frag_senc = current_frag_senc.clone();
            let fragments = fragments.clone();
            move |box_| {
                *current_frag_track_id.borrow_mut() = 0;
                *current_frag_trun.borrow_mut() = None;
                *current_frag_senc.borrow_mut() = None;

                parser::children(box_)?;

                let tid = *current_frag_track_id.borrow();
                let trun = current_frag_trun.borrow_mut().take();
                let senc = current_frag_senc.borrow_mut().take();

                if let (Some(trun), Some(senc)) = (trun, senc) {
                    fragments.borrow_mut().push(TrackFragment {
                        track_id: tid,
                        trun,
                        senc,
                    });
                }
                Ok(())
            }
        })
        .full_box("tfhd", {
            let current_frag_track_id = current_frag_track_id.clone();
            move |mut box_| {
                let tfhd = TfhdBox::new(&mut box_)?;
                *current_frag_track_id.borrow_mut() = tfhd.track_id;
                Ok(())
            }
        })
        .full_box("tfdt", |_| Ok(()))
        .full_box("trun", {
            let current_frag_trun = current_frag_trun.clone();
            move |mut box_| {
                *current_frag_trun.borrow_mut() = Some(TrunBox::new(&mut box_)?);
                Ok(())
            }
        })
        .full_box("senc", {
            let current_frag_track_id = current_frag_track_id.clone();
            let current_frag_senc = current_frag_senc.clone();
            move |mut box_| {
                let tid = *current_frag_track_id.borrow();
                let (iv_size, constant_iv) = if let Some((iv, civ)) = iv_info.get(&tid) {
                    (*iv, civ.clone())
                } else {
                    (8, None)
                };
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

    for frag in frags.iter() {
        let track_info = match tracks.get(&frag.track_id) {
            Some(info) => info,
            None => continue,
        };

        let kid = track_info.tenc.default_kid;
        let key = keys
            .get(&kid)
            .ok_or_else(|| DecryptError::KeyNotFound(hex::encode(kid)))?;

        let mut decrypter = Decrypter::new(
            track_info.scheme_type,
            key,
            track_info.tenc.crypt_byte_block,
            track_info.tenc.skip_byte_block,
        );

        let data_start = {
            let offset = frag.trun.data_offset.unwrap_or_default() as i64;
            (moof_start_val as i64 + offset) as usize
        };

        let mut offset = data_start;
        let output_len = input_data.len();

        for (trun_sample, senc_sample) in frag.trun.sample_data.iter().zip(frag.senc.samples.iter())
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

fn parse_hex_16(input: &str) -> Result<[u8; 16]> {
    hex::decode(input.to_ascii_lowercase().replace('-', ""))?
        .try_into()
        .map_err(|v: Vec<u8>| DecryptError::HexWrongLength(v.len()))
}
