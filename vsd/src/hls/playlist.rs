use crate::{playlist, utils};

pub fn parse_as_master(base_url: &str, m3u8: &m3u8_rs::MasterPlaylist) -> playlist::MasterPlaylist {
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
    let start = br.offset.unwrap_or(*next_byte);
    let end = start + br.length - 1;
    *next_byte = end + 1;
    playlist::Range(start, end)
}

pub fn push_segments(stream: &mut playlist::MediaPlaylist, m3u8: m3u8_rs::MediaPlaylist) {
    stream.i_frame = m3u8.i_frames_only;
    stream.live = !m3u8.end_list;
    stream.media_sequence = m3u8.media_sequence;

    let mut next_byte = 0;

    for segment in m3u8.segments {
        let map = segment.map.map(|x| playlist::Map {
            uri: x.uri,
            range: x.byte_range.map(|br| parse_byte_range(&br, &mut next_byte)),
        });

        let range = segment
            .byte_range
            .map(|br| parse_byte_range(&br, &mut next_byte));

        let key = segment.key.map(|key| {
            let mut method = match &key.method {
                m3u8_rs::KeyMethod::AES128 => playlist::KeyMethod::Aes128,
                m3u8_rs::KeyMethod::None => playlist::KeyMethod::None,
                m3u8_rs::KeyMethod::SampleAES => playlist::KeyMethod::SampleAes,
                m3u8_rs::KeyMethod::Other(x) if x == "SAMPLE-AES-CENC" || x == "SAMPLE-AES-CTR" => {
                    playlist::KeyMethod::Cenc
                }
                m3u8_rs::KeyMethod::Other(x) => playlist::KeyMethod::Other(x.to_owned()),
            };

            if method == playlist::KeyMethod::None
                && let Some(keyformat) = key.keyformat.as_deref()
            {
                if matches!(
                    keyformat,
                    "urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"
                        | "urn:uuid:9a04f079-9840-4286-ab92-e65be0885f95"
                        | "com.apple.streamingkeydelivery"
                ) {
                    method = playlist::KeyMethod::Cenc;
                }
            }

            if method == playlist::KeyMethod::None
                && let Some(uri) = key.uri.as_ref()
                && (uri.starts_with("data:text/plain;base64,") || uri.starts_with("skd://"))
            {
                method = playlist::KeyMethod::Cenc;
            }

            playlist::Key {
                default_kid: None,
                iv: key.iv,
                method,
                uri: key.uri,
            }
        });

        stream.segments.push(playlist::Segment {
            duration: segment.duration,
            key,
            map,
            range,
            uri: segment.uri,
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
