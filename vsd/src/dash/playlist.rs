/*
    REFERENCES
    ----------

    1. https://github.com/emarsden/dash-mpd-rs/blob/7e985069fd95fd5d9993b7610c28228d2448aea7/src/fetch.rs#L2428-L2870

*/

use super::{DashUrl, Template};
use crate::{
    playlist::{
        Key, KeyMethod, Map, MasterPlaylist, MediaPlaylist, MediaType, PlaylistType, Range, Segment,
    },
    utils,
};
use anyhow::{Result, bail};
use dash_mpd::MPD;
use log::debug;
use reqwest::{Client, Url, header};
use std::collections::HashMap;
use vsd_mp4::boxes::SidxBox;

fn parse_frame_rate(frame_rate: &str) -> Option<f32> {
    if frame_rate.contains('/') {
        frame_rate.split_once('/').and_then(|(x, y)| {
            if let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>()) {
                Some(x / y)
            } else {
                None
            }
        })
    } else {
        frame_rate.parse::<f32>().ok()
    }
}

fn parse_range(range: &Option<String>) -> Option<Range> {
    range.as_ref().and_then(|range| {
        range.split_once('-').and_then(|(x, y)| {
            if let (Ok(x), Ok(y)) = (x.parse::<u64>(), y.parse::<u64>()) {
                Some(Range { end: y, start: x })
            } else {
                None
            }
        })
    })
}

pub(crate) fn parse_as_master(base_url: &str, mpd: &MPD) -> MasterPlaylist {
    let mut playlist = MasterPlaylist {
        playlist_type: PlaylistType::Dash,
        streams: Vec::new(),
        uri: base_url.to_string(),
    };

    let Some(period) = mpd.periods.first() else {
        return playlist;
    };

    for (adaptation_index, adaptation_set) in period.adaptations.iter().enumerate() {
        for (representation_index, representation) in
            adaptation_set.representations.iter().enumerate()
        {
            let codecs = representation
                .codecs
                .clone()
                .or(adaptation_set.codecs.clone());

            let mime_type = representation
                .mimeType
                .clone()
                .or(adaptation_set.mimeType.clone())
                .or(representation.contentType.clone())
                .or(adaptation_set.contentType.clone());

            let mut media_type = if let Some(mime_type) = &mime_type {
                match mime_type.as_str() {
                    "application/ttml+xml" | "application/x-sami" => MediaType::Subtitles,
                    x if x.starts_with("audio") => MediaType::Audio,
                    x if x.starts_with("text") => MediaType::Subtitles,
                    x if x.starts_with("video") => MediaType::Video,
                    _ => MediaType::Undefined,
                }
            } else {
                MediaType::Undefined
            };

            if media_type == MediaType::Undefined
                && let Some(codecs) = &codecs
            {
                media_type = match codecs.as_str() {
                    "wvtt" | "stpp" => MediaType::Subtitles,
                    x if x.starts_with("stpp.") => MediaType::Subtitles,
                    _ => media_type,
                };
            }

            playlist.streams.push(MediaPlaylist {
                bandwidth: representation.bandwidth,
                channels: representation
                    .AudioChannelConfiguration
                    .first()
                    .and_then(|x| x.value.as_ref().and_then(|y| y.parse::<f32>().ok()))
                    .or(adaptation_set
                        .AudioChannelConfiguration
                        .first()
                        .and_then(|x| x.value.as_ref().and_then(|y| y.parse::<f32>().ok()))),
                codecs,
                extension: mime_type
                    .as_ref()
                    .and_then(|x| x.split_once('/').map(|(_, y)| y.to_owned())),
                frame_rate: representation
                    .frameRate
                    .as_ref()
                    .and_then(|x| parse_frame_rate(x))
                    .or(adaptation_set
                        .frameRate
                        .as_ref()
                        .and_then(|x| parse_frame_rate(x))),
                id: utils::gen_id(
                    base_url,
                    &DashUrl::new(0, adaptation_index, representation_index).to_string(),
                ),
                i_frame: false,
                language: adaptation_set.lang.clone(),
                live: mpd
                    .mpdtype
                    .as_ref()
                    .map(|x| x == "dynamic")
                    .unwrap_or(false),
                media_sequence: 0,
                media_type,
                playlist_type: PlaylistType::Dash,
                resolution: if let (Some(width), Some(height)) =
                    (representation.width, representation.height)
                {
                    Some((width, height))
                } else {
                    None
                },
                segments: Vec::new(),
                uri: DashUrl::new(0, adaptation_index, representation_index).to_string(),
            });
        }
    }

    playlist
}

pub(crate) async fn push_segments(
    client: &Client,
    base_url: &str,
    query: &Vec<(String, String)>,
    playlist: &MPD,
    stream: &mut MediaPlaylist,
) -> Result<()> {
    let location = stream.uri.parse::<DashUrl>().unwrap();

    for period in playlist.periods.iter() {
        for (adaptation_index, adaptation_set) in period.adaptations.iter().enumerate() {
            for (representation_index, representation) in
                adaptation_set.representations.iter().enumerate()
            {
                if adaptation_index == location.adaptation_set
                    && representation_index == location.representation
                {
                    let mut period_duration_secs = 0.0;

                    if let Some(duration) = &playlist.mediaPresentationDuration {
                        period_duration_secs = duration.as_secs_f32();
                    }

                    if let Some(duration) = &period.duration {
                        period_duration_secs = duration.as_secs_f32();
                    }

                    let mut base_url = base_url.parse::<Url>()?;

                    if let Some(mpd_baseurl) = playlist.base_url.first().map(|x| x.base.as_ref()) {
                        base_url = base_url.join(mpd_baseurl)?;
                    }

                    if let Some(period_baseurl) = period.BaseURL.first().map(|x| x.base.as_ref()) {
                        base_url = base_url.join(period_baseurl)?;
                    }

                    if let Some(adaptation_set_baseurl) =
                        adaptation_set.BaseURL.first().map(|x| x.base.as_ref())
                    {
                        base_url = base_url.join(adaptation_set_baseurl)?;
                    }

                    if let Some(representation_baseurl) =
                        representation.BaseURL.first().map(|x| x.base.as_ref())
                    {
                        base_url = base_url.join(representation_baseurl)?;
                    }

                    let mut init_map = None;

                    let rid = if let Some(id) = &representation.id {
                        id.to_owned()
                    } else {
                        bail!("missing @id on representation node.");
                    };

                    let mut template_vars = HashMap::from([("RepresentationID".to_owned(), rid)]);

                    if let Some(bandwidth) = &representation.bandwidth {
                        template_vars.insert("Bandwidth".to_owned(), bandwidth.to_string());
                    }

                    let mut template = Template::new(template_vars);

                    // Now the 6 possible addressing modes:
                    // (1.1) AdaptationSet>SegmentList
                    // (1.2) Representation>SegmentList
                    // ( 2 ) SegmentTemplate+SegmentTimeline
                    // ( 3 ) SegmentTemplate@duration
                    // ( 4 ) SegmentTemplate@index
                    // ( 5 ) SegmentBase@indexRange
                    // ( 6 ) Plain BaseURL

                    // Though SegmentBase and SegmentList addressing modes are supposed to be
                    // mutually exclusive, some manifests in the wild use both. So we try to work
                    // around the brokenness.

                    // (1.1) AdaptationSet>SegmentList
                    if let Some(segment_list) = &adaptation_set.SegmentList {
                        if let Some(initialization) = &segment_list.Initialization {
                            let byte_range = parse_range(&initialization.range);

                            if let Some(source_url) = &initialization.sourceURL {
                                init_map = Some(Map {
                                    range: byte_range,
                                    uri: base_url.join(&template.resolve(source_url))?.to_string(),
                                });
                            } else {
                                init_map = Some(Map {
                                    range: byte_range,
                                    uri: base_url.to_string(),
                                });
                            }
                        }

                        for segment_url in &segment_list.segment_urls {
                            // We are ignoring SegmentURL@indexRange
                            let byte_range = parse_range(&segment_url.mediaRange);

                            if let Some(media) = &segment_url.media {
                                stream.segments.push(Segment {
                                    range: byte_range,
                                    uri: base_url.join(media)?.to_string(),
                                    ..Default::default()
                                });
                            } else if !adaptation_set.BaseURL.is_empty() {
                                stream.segments.push(Segment {
                                    range: byte_range,
                                    uri: base_url.to_string(),
                                    ..Default::default()
                                });
                            }
                        }
                    }

                    // (1.2) Representation>SegmentList
                    if let Some(segment_list) = &representation.SegmentList {
                        if let Some(initialization) = &segment_list.Initialization {
                            let byte_range = parse_range(&initialization.range);

                            if let Some(source_url) = &initialization.sourceURL {
                                init_map = Some(Map {
                                    range: byte_range,
                                    uri: base_url.join(&template.resolve(source_url))?.to_string(),
                                });
                            } else {
                                init_map = Some(Map {
                                    range: byte_range,
                                    uri: base_url.to_string(),
                                });
                            }
                        }

                        for segment_url in &segment_list.segment_urls {
                            // We are ignoring SegmentURL@indexRange
                            let byte_range = parse_range(&segment_url.mediaRange);

                            if let Some(media) = &segment_url.media {
                                stream.segments.push(Segment {
                                    range: byte_range,
                                    uri: base_url.join(media)?.to_string(),
                                    ..Default::default()
                                });
                            } else if !representation.BaseURL.is_empty() {
                                stream.segments.push(Segment {
                                    range: byte_range,
                                    uri: base_url.to_string(),
                                    ..Default::default()
                                });
                            }
                        }
                    } else if representation.SegmentTemplate.is_some()
                        || adaptation_set.SegmentTemplate.is_some()
                    {
                        let segment_template = representation
                            .SegmentTemplate
                            .as_ref()
                            .or(adaptation_set.SegmentTemplate.as_ref())
                            .unwrap();

                        if let Some(initialization) = &segment_template.initialization {
                            init_map = Some(Map {
                                range: None,
                                uri: base_url
                                    .join(&template.resolve(initialization))?
                                    .to_string(),
                            });
                        }

                        // (2) SegmentTemplate+SegmentTimeline (explicit addressing)
                        if let Some(segment_timeline) = &segment_template.SegmentTimeline {
                            if segment_template.media.is_none() {
                                bail!("SegmentTimeline without a media attribute.");
                            }

                            let media = template.resolve(segment_template.media.as_ref().unwrap());
                            let mut number = segment_template.startNumber.unwrap_or(1);
                            let mut segment_time = 0;
                            let timescale = segment_template.timescale.unwrap_or(1) as f32;

                            for s in &segment_timeline.segments {
                                if let Some(t) = s.t {
                                    segment_time = t;
                                }

                                template.insert("Time", segment_time.to_string());
                                template.insert("Number", number.to_string());

                                stream.segments.push(Segment {
                                    duration: s.d as f32 / timescale,
                                    uri: base_url.join(&template.resolve(&media))?.to_string(),
                                    ..Default::default()
                                });

                                number += 1;

                                if let Some(r) = s.r {
                                    let mut count = 0;
                                    // FIXME - Perhaps we also need to account for startTime?
                                    let end_time = period_duration_secs * timescale;

                                    loop {
                                        count += 1;
                                        // Exit from the loop after @r iterations (if @r is
                                        // positive). A negative value of the @r attribute indicates
                                        // that the duration indicated in @d attribute repeats until
                                        // the start of the next S element, the end of the Period or
                                        // until the next MPD update.
                                        if r >= 0 {
                                            if count > r {
                                                break;
                                            }
                                        } else if segment_time as f32 > end_time {
                                            break;
                                        }

                                        segment_time += s.d;

                                        template.insert("Time", segment_time.to_string());
                                        template.insert("Number", number.to_string());

                                        stream.segments.push(Segment {
                                            duration: s.d as f32 / timescale,
                                            uri: base_url
                                                .join(&template.resolve(&media))?
                                                .to_string(),
                                            ..Default::default()
                                        });

                                        number += 1;
                                    }
                                }

                                segment_time += s.d;
                            }
                        } else if let Some(media) = &segment_template.media {
                            // (3) SegmentTemplate@duration || (4) SegmentTemplate@index (simple addressing)
                            let mut segment_duration = -1.0;
                            let media = template.resolve(media);
                            let timescale = segment_template.timescale.unwrap_or(1) as f32;

                            if let Some(x) = segment_template.duration {
                                segment_duration = x as f32 / timescale;
                            }

                            if segment_duration < 0.0 {
                                bail!(
                                    "Representation is missing SegmentTemplate@duration attribute."
                                );
                            }

                            let number = segment_template.startNumber.unwrap_or(1) as i64;
                            let total_number =
                                number + (period_duration_secs / segment_duration).round() as i64;

                            // // For a live manifest (dynamic MPD), we look at the time elapsed since now
                            // // and the mpd.availabilityStartTime to determine the correct value for
                            // // startNumber, based on duration and timescale. The latest available
                            // // segment is numbered
                            // //
                            // //    LSN = floor((now - (availabilityStartTime+PST))/segmentDuration + startNumber - 1)

                            // // https://dashif.org/Guidelines-TimingModel/Timing-Model.pdf
                            // // To be more precise, any LeapSecondInformation should be added to the availabilityStartTime.
                            // if mpd_is_dynamic(mpd) {
                            //     if let Some(start_time) = mpd.availabilityStartTime {
                            //         let elapsed = Utc::now()
                            //             .signed_duration_since(start_time)
                            //             .as_seconds_f64()
                            //             / segment_duration;
                            //         number = (elapsed + number as f64 - 1f64).floor() as u64;
                            //     } else {
                            //         return Err(DashMpdError::UnhandledMediaStream(
                            //             "dynamic manifest is missing @availabilityStartTime"
                            //                 .to_string(),
                            //         ));
                            //     }
                            // }

                            for number in
                                segment_template.startNumber.unwrap_or(1) as i64..=total_number
                            {
                                template.insert("Number", number.to_string());

                                stream.segments.push(Segment {
                                    duration: segment_duration,
                                    uri: base_url.join(&template.resolve(&media))?.to_string(),
                                    ..Default::default()
                                });
                            }
                        }
                    } else if let Some(segment_base) = &representation.SegmentBase {
                        // (5) SegmentBase@indexRange
                        if let Some(initialization) = &segment_base.Initialization {
                            let byte_range = parse_range(&initialization.range);

                            if let Some(source_url) = &initialization.sourceURL {
                                init_map = Some(Map {
                                    range: byte_range,
                                    uri: base_url.join(&template.resolve(source_url))?.to_string(),
                                });
                            } else {
                                init_map = Some(Map {
                                    range: byte_range,
                                    uri: base_url.to_string(),
                                });
                            }
                        }

                        if let Some(index_range) = parse_range(&segment_base.indexRange) {
                            debug!(
                                "Fetching {} (sidx@{}-{})",
                                base_url, index_range.start, index_range.end
                            );
                            let request = client
                                .get(base_url.as_str())
                                .query(query)
                                .header(header::RANGE, &index_range);
                            let response = request.send().await?;
                            let bytes = utils::fetch_bytes(response).await?;

                            if let Some(init_map) = &mut init_map {
                                init_map.range = Some(Range {
                                    end: index_range.end,
                                    start: 0,
                                })
                            }

                            for range in SidxBox::from_init(&bytes, index_range.start)?
                                .map(|x| x.ranges)
                                .unwrap_or(Vec::with_capacity(0))
                            {
                                stream.segments.push(Segment {
                                    range: Some(Range {
                                        end: range.end,
                                        start: range.start,
                                    }),
                                    uri: base_url.to_string(),
                                    ..Default::default()
                                });
                            }
                        } else {
                            stream.segments.push(Segment {
                                uri: base_url.to_string(),
                                ..Default::default()
                            });
                        }
                    } else if stream.segments.is_empty() && !representation.BaseURL.is_empty() {
                        // (6) Plain BaseURL
                        stream.segments.push(Segment {
                            duration: period_duration_secs,
                            uri: base_url.to_string(),
                            ..Default::default()
                        });
                    }

                    if stream.segments.is_empty() {
                        bail!("no usable addressing mode identified for representation.");
                    }

                    if let Some(first_segment) = stream.segments.get_mut(0) {
                        let mut encryption_type = KeyMethod::None;
                        let mut default_kid = None;

                        for content_protection in &representation.ContentProtection {
                            if default_kid.is_none() && content_protection.default_KID.is_some() {
                                default_kid = content_protection.default_KID.clone();
                            }

                            // content_protection.value = "cenc" | "cbcs" | "cens" | "cbc1"
                            if encryption_type == KeyMethod::None
                                && content_protection.value.is_some()
                            {
                                encryption_type = KeyMethod::Cenc;
                            }
                        }

                        if encryption_type == KeyMethod::None || default_kid.is_none() {
                            for content_protection in &adaptation_set.ContentProtection {
                                if default_kid.is_none() && content_protection.default_KID.is_some()
                                {
                                    default_kid = content_protection.default_KID.clone();
                                }

                                if encryption_type == KeyMethod::None
                                    && content_protection.value.is_some()
                                {
                                    encryption_type = KeyMethod::Cenc;
                                }
                            }
                        }

                        default_kid = default_kid.map(|x| x.to_lowercase());

                        first_segment.key = match encryption_type {
                            KeyMethod::None => None,
                            x => Some(Key {
                                default_kid,
                                iv: None,
                                key_format: None,
                                method: x,
                                uri: None,
                            }),
                        };

                        first_segment.map = init_map;
                    }
                }
            }
        }
    }

    stream.uri = base_url.to_owned();
    Ok(())
}
