use crate::{playlist, utils};

pub(crate) fn parse_as_master(
    base_url: &str,
    m3u8: &m3u8_rs::MasterPlaylist,
) -> playlist::MasterPlaylist {
    let mut streams = Vec::new();

    for stream in &m3u8.variants {
        streams.push(playlist::MediaPlaylist {
            bandwidth: Some(stream.bandwidth),
            codecs: stream.codecs.to_owned(),
            extension: Some("ts".to_owned()),
            frame_rate: stream.frame_rate.map(|x| x as f32),
            id: utils::gen_id(base_url, &stream.uri),
            i_frame: stream.is_i_frame,
            media_type: playlist::MediaType::Video,
            playlist_type: playlist::PlaylistType::Hls,
            resolution: stream.resolution.map(|r| (r.width, r.height)),
            uri: stream.uri.to_owned(),
            ..Default::default()
        });
    }

    for alt in &m3u8.alternatives {
        let Some(uri) = &alt.uri else {
            continue;
        };

        streams.push(playlist::MediaPlaylist {
            bandwidth: alt.other_attributes.as_ref().and_then(|x| {
                x.get("BANDWIDTH")
                    .and_then(|x| x.as_str().parse::<u64>().ok())
            }),
            channels: alt.channels.as_ref().and_then(|x| x.parse::<f32>().ok()),
            codecs: alt
                .other_attributes
                .as_ref()
                .and_then(|x| x.get("CODECS").map(|x| x.as_str().to_owned())),
            extension: match alt.media_type {
                m3u8_rs::AlternativeMediaType::ClosedCaptions
                | m3u8_rs::AlternativeMediaType::Subtitles => Some("vtt".to_owned()),
                _ => Some("ts".to_owned()),
            },
            id: utils::gen_id(base_url, uri),
            language: alt.language.clone().or(alt.assoc_language.clone()),
            media_type: match alt.media_type {
                m3u8_rs::AlternativeMediaType::Audio => playlist::MediaType::Audio,
                m3u8_rs::AlternativeMediaType::ClosedCaptions
                | m3u8_rs::AlternativeMediaType::Subtitles => playlist::MediaType::Subtitles,
                m3u8_rs::AlternativeMediaType::Other(_) => playlist::MediaType::Undefined,
                m3u8_rs::AlternativeMediaType::Video => playlist::MediaType::Video,
            },
            playlist_type: playlist::PlaylistType::Hls,
            uri: uri.to_owned(),
            ..Default::default()
        });
    }

    playlist::MasterPlaylist {
        playlist_type: playlist::PlaylistType::Hls,
        uri: base_url.to_owned(),
        streams,
    }
}

fn parse_byte_range(br: &m3u8_rs::ByteRange, next_byte: &mut u64) -> playlist::Range {
    let (start, end) = if let Some(offset) = br.offset {
        (offset, offset + br.length - 1)
    } else {
        (*next_byte, *next_byte + br.length - 1)
    };
    *next_byte = end + 1;
    playlist::Range { start, end }
}

pub(crate) fn push_segments(
    playlist: &m3u8_rs::MediaPlaylist,
    stream: &mut playlist::MediaPlaylist,
) {
    stream.i_frame = playlist.i_frames_only;
    stream.live = !playlist.end_list;
    stream.media_sequence = playlist.media_sequence;

    let mut next_byte: u64 = 0;

    for segment in &playlist.segments {
        let map = segment.map.as_ref().map(|x| playlist::Map {
            uri: x.uri.to_owned(),
            range: x
                .byte_range
                .as_ref()
                .map(|br| parse_byte_range(br, &mut next_byte)),
        });

        let range = segment
            .byte_range
            .as_ref()
            .map(|br| parse_byte_range(br, &mut next_byte));

        stream.segments.push(playlist::Segment {
            duration: segment.duration,
            key: if let Some(m3u8_rs::Key {
                iv,
                keyformat,
                method,
                uri,
                ..
            }) = &segment.key
            {
                let mut method = match method {
                    m3u8_rs::KeyMethod::AES128 => playlist::KeyMethod::Aes128,
                    m3u8_rs::KeyMethod::None => playlist::KeyMethod::None, // This should never match according to hls specifications.
                    m3u8_rs::KeyMethod::SampleAES => playlist::KeyMethod::SampleAes,
                    m3u8_rs::KeyMethod::Other(x)
                        if x == "SAMPLE-AES-CENC" || x == "SAMPLE-AES-CTR" =>
                    {
                        playlist::KeyMethod::Cenc
                    }
                    m3u8_rs::KeyMethod::Other(x) => playlist::KeyMethod::Other(x.to_owned()),
                };

                /*
                    .mpd (with encryption) converted to .m3u8

                    #EXT-X-KEY:METHOD=SAMPLE-AES,URI="skd://302f80dd-411e-4886-bca5-bb1f8018a024:77FD1889AAF4143B085548B3C0F95B9A",KEYFORMATVERSIONS="1",KEYFORMAT="com.apple.streamingkeydelivery"
                    #EXT-X-KEY:METHOD=SAMPLE-AES-CTR,KEYFORMAT="com.microsoft.playready",KEYFORMATVERSIONS="1",URI="data:text/plain;charset=UTF-16;base64,xAEAAAEAAQC6ATwAVwBSAE0ASABFAEEARABFAFIAIAB4AG0AbABuAHMAPQAiAGgAdAB0AHAAOgAvAC8AcwBjAGgAZQBtAGEAcwAuAG0AaQBjAHIAbwBzAG8AZgB0AC4AYwBvAG0ALwBEAFIATQAvADIAMAAwADcALwAwADMALwBQAGwAYQB5AFIAZQBhAGQAeQBIAGUAYQBkAGUAcgAiACAAdgBlAHIAcwBpAG8AbgA9ACIANAAuADAALgAwAC4AMAAiAD4APABEAEEAVABBAD4APABQAFIATwBUAEUAQwBUAEkATgBGAE8APgA8AEsARQBZAEwARQBOAD4AMQA2ADwALwBLAEUAWQBMAEUATgA+ADwAQQBMAEcASQBEAD4AQQBFAFMAQwBUAFIAPAAvAEEATABHAEkARAA+ADwALwBQAFIATwBUAEUAQwBUAEkATgBGAE8APgA8AEsASQBEAD4AOQBmAEIAMQAxAEsAMQB0AC8ARQBtAFEANABYAEMATQBjAEoANgBnAEkAZwA9AD0APAAvAEsASQBEAD4APAAvAEQAQQBUAEEAPgA8AC8AVwBSAE0ASABFAEEARABFAFIAPgA="
                    #EXT-X-KEY:METHOD=SAMPLE-AES,URI="data:text/plain;base64,AAAAXHBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAADwSEDAvgN1BHkiGvKW7H4AYoCQSEDAvgN1BHkiGvKW7H4AYoCQSEDAvgN1BHkiGvKW7H4AYoCRI88aJmwY=",KEYID=0x302F80DD411E4886BCA5BB1F8018A024,IV=0x77FD1889AAF4143B085548B3C0F95B9A,KEYFORMATVERSIONS="1",KEYFORMAT="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"

                    https://dashif.org/identifiers/content_protection
                */
                if let Some(keyformat) = keyformat {
                    method = match keyformat.as_str() {
                        "com.apple.streamingkeydelivery"
                        | "com.microsoft.playready"
                        | "urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed" => {
                            playlist::KeyMethod::Cenc
                        }
                        _ => method,
                    };
                }

                Some(playlist::Key {
                    default_kid: None,
                    iv: iv.clone(),
                    key_format: keyformat.clone(),
                    method,
                    uri: uri.clone(),
                })
            } else {
                None
            },
            map,
            range,
            uri: segment.uri.to_owned(),
        });
    }

    if let Some(segment) = stream.segments.first() {
        if let Some(init) = &segment.map
            && init.uri.split('?').next().unwrap().ends_with(".mp4")
        {
            stream.extension = Some("m4s".to_owned());
        }

        let uri = segment.uri.split('?').next().unwrap();

        if uri.ends_with(".mp4") || uri.ends_with(".m4s") {
            stream.extension = Some("m4s".to_owned());
        }
    }
}
