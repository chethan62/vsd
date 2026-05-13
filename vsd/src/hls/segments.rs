use crate::playlist;

fn parse_byte_range(br: &m3u8_rs::ByteRange, next_byte: &mut u64) -> playlist::Range {
    let start = br.offset.unwrap_or(*next_byte);
    let end = start + br.length - 1;
    *next_byte = end + 1;
    playlist::Range(start, end)
}

fn parse_key(key: m3u8_rs::Key) -> playlist::Key {
    let mut method = match &key.method {
        m3u8_rs::KeyMethod::AES128 => playlist::KeyMethod::Aes128,
        m3u8_rs::KeyMethod::None => playlist::KeyMethod::None,
        m3u8_rs::KeyMethod::SampleAES => playlist::KeyMethod::SampleAes,
        m3u8_rs::KeyMethod::Other(x) if x == "SAMPLE-AES-CENC" || x == "SAMPLE-AES-CTR" => {
            playlist::KeyMethod::Cenc
        }
        m3u8_rs::KeyMethod::Other(x) => playlist::KeyMethod::Other(x.to_owned()),
    };

    if method != playlist::KeyMethod::Cenc
        && let Some(keyformat) = key.keyformat.as_deref()
        && matches!(
            keyformat,
            "urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"
                | "urn:uuid:9a04f079-9840-4286-ab92-e65be0885f95"
                | "com.apple.streamingkeydelivery"
        )
    {
        method = playlist::KeyMethod::Cenc;
    }

    if method != playlist::KeyMethod::Cenc
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

        stream.segments.push(playlist::Segment {
            duration: segment.duration,
            key: segment.key.map(parse_key),
            map,
            range,
            uri: segment.uri,
        });
    }

    if let Some(first) = stream.segments.first() {
        let is_mp4 = |uri: &str| {
            let path = uri.split_once('?').map_or(uri, |(p, _)| p);
            path.ends_with(".mp4") || path.ends_with(".m4s")
        };

        if is_mp4(&first.uri) || first.map.as_ref().is_some_and(|m| is_mp4(&m.uri)) {
            stream.extension = Some("mp4".to_owned());
        }
    }
}
