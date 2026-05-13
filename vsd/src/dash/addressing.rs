use crate::{
    dash::{Template, parse_range},
    playlist::{Map, Range, Segment},
    utils,
};
use anyhow::Result;
use dash_mpd::SegmentTemplate;
use log::debug;
use reqwest::{Client, Url, header};
use vsd_mp4::boxes::SidxBox;

pub fn parse_init(
    initialization: &dash_mpd::Initialization,
    base_url: &Url,
    template: &Template,
) -> Result<Map> {
    Ok(Map {
        range: parse_range(&initialization.range),
        uri: if let Some(source_url) = &initialization.sourceURL {
            base_url.join(&template.resolve(source_url))?.to_string()
        } else {
            base_url.to_string()
        },
    })
}

pub fn process_segment_list(
    segment_list: &dash_mpd::SegmentList,
    base_url: &Url,
    template: &Template,
    has_base_url: bool,
) -> Result<Vec<Segment>> {
    let mut segments = Vec::new();

    for segment_url in &segment_list.segment_urls {
        // We are ignoring SegmentURL@indexRange
        let byte_range = parse_range(&segment_url.mediaRange);

        if let Some(media) = &segment_url.media {
            segments.push(Segment {
                range: byte_range,
                uri: base_url.join(media)?.to_string(),
                ..Default::default()
            });
        } else if has_base_url {
            segments.push(Segment {
                range: byte_range,
                uri: base_url.to_string(),
                ..Default::default()
            });
        }
    }

    if let (Some(first), Some(initialization)) =
        (segments.first_mut(), &segment_list.Initialization)
    {
        first.map = Some(parse_init(initialization, base_url, template)?);
    }

    Ok(segments)
}

// ─── SegmentTemplate + SegmentTimeline ──────────────────────────────────────

/// Process SegmentTemplate with an explicit SegmentTimeline.
pub fn process_segment_timeline(
    segment_timeline: &dash_mpd::SegmentTimeline,
    tmpl_media: &str,
    tmpl_start_number: u64,
    tmpl_timescale: u64,
    period_duration_secs: f64,
    base_url: &Url,
    template: &mut Template,
) -> Result<Vec<Segment>> {
    let mut segments = Vec::new();
    let media = template.resolve(tmpl_media);
    let mut number = tmpl_start_number;
    let mut segment_time: u64 = 0;
    let timescale = tmpl_timescale as f64;

    for s in &segment_timeline.segments {
        if let Some(t) = s.t {
            segment_time = t;
        }

        template.insert("Time", segment_time.to_string());
        template.insert("Number", number.to_string());

        segments.push(Segment {
            duration: (s.d as f64 / timescale) as f32,
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
                // Exit from the loop after @r iterations (if @r is positive).
                // A negative value of the @r attribute indicates that the duration
                // indicated in @d attribute repeats until the start of the next S
                // element, the end of the Period or until the next MPD update.
                if r >= 0 {
                    if count > r {
                        break;
                    }
                } else if segment_time as f64 > end_time {
                    break;
                }

                segment_time += s.d;

                template.insert("Time", segment_time.to_string());
                template.insert("Number", number.to_string());

                segments.push(Segment {
                    duration: (s.d as f64 / timescale) as f32,
                    uri: base_url.join(&template.resolve(&media))?.to_string(),
                    ..Default::default()
                });

                number += 1;
            }
        }

        segment_time += s.d;
    }

    Ok(segments)
}

// ─── SegmentTemplate@duration ───────────────────────────────────────────────

/// Process SegmentTemplate with @duration (simple segment numbering).
pub fn process_segment_template_duration(
    tmpl_media: &str,
    tmpl_start_number: u64,
    tmpl_timescale: u64,
    tmpl_duration: f64,
    period_duration_secs: f64,
    base_url: &Url,
    template: &mut Template,
) -> Result<Vec<Segment>> {
    let mut segments = Vec::new();
    let media = template.resolve(tmpl_media);
    let timescale = tmpl_timescale as f64;
    let segment_duration = tmpl_duration / timescale;

    let start_number = tmpl_start_number as i64;
    let segment_count = (period_duration_secs / segment_duration).ceil() as i64;

    for number in start_number..start_number + segment_count {
        template.insert("Number", number.to_string());

        segments.push(Segment {
            duration: segment_duration as f32,
            uri: base_url.join(&template.resolve(&media))?.to_string(),
            ..Default::default()
        });
    }

    Ok(segments)
}

// ─── SegmentBase@indexRange ─────────────────────────────────────────────────

/// Process SegmentBase with @indexRange (fetch SIDX box for byte ranges).
pub async fn process_segment_base(
    segment_base: &dash_mpd::SegmentBase,
    base_url: &Url,
    template: &Template,
    client: &Client,
    query: &[(String, String)],
) -> Result<Vec<Segment>> {
    let mut segments = Vec::new();

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

        for range in SidxBox::from_init(&bytes, index_range.start)?
            .map(|x| x.ranges)
            .unwrap_or_default()
        {
            segments.push(Segment {
                range: Some(Range {
                    end: range.end,
                    start: range.start,
                }),
                uri: base_url.to_string(),
                ..Default::default()
            });
        }

        // Init map covers bytes 0 through end of SIDX
        if let Some(first) = segments.first_mut() {
            if let Some(initialization) = &segment_base.Initialization {
                let mut map = parse_init(initialization, base_url, template)?;
                map.range = Some(Range {
                    end: index_range.end,
                    start: 0,
                });
                first.map = Some(map);
            }
        }
    } else {
        segments.push(Segment {
            uri: base_url.to_string(),
            map: segment_base
                .Initialization
                .as_ref()
                .map(|init| parse_init(init, base_url, template))
                .transpose()?,
            ..Default::default()
        });
    }

    Ok(segments)
}

// ─── SegmentTemplate init resolution ────────────────────────────────────────

/// Resolve initialization for SegmentTemplate addressing modes.
/// Checks @initialization attribute first, then <Initialization> child element,
/// merging from Representation and AdaptationSet levels.
pub fn resolve_segment_template_init(
    repr_tmpl: Option<&SegmentTemplate>,
    adapt_tmpl: Option<&SegmentTemplate>,
    base_url: &Url,
    template: &Template,
) -> Result<Option<Map>> {
    // Try @initialization attribute
    let tmpl_initialization = merge_tmpl_field(repr_tmpl, adapt_tmpl, |t| t.initialization.clone());

    if let Some(initialization) = tmpl_initialization {
        return Ok(Some(Map {
            range: None,
            uri: base_url
                .join(&template.resolve(&initialization))?
                .to_string(),
        }));
    }

    // Try <Initialization> child element
    let tmpl_init_element = repr_tmpl
        .and_then(|t| t.Initialization.as_ref())
        .or(adapt_tmpl.and_then(|t| t.Initialization.as_ref()));

    if let Some(initialization) = tmpl_init_element {
        return Ok(Some(parse_init(initialization, base_url, template)?));
    }

    Ok(None)
}

/// Resolve merged SegmentTemplate field: uses Representation's value if present,
/// else falls back to AdaptationSet's value.
pub fn merge_tmpl_field<T: Clone>(
    repr_tmpl: Option<&SegmentTemplate>,
    adapt_tmpl: Option<&SegmentTemplate>,
    getter: fn(&SegmentTemplate) -> Option<T>,
) -> Option<T> {
    repr_tmpl
        .and_then(|t| getter(t))
        .or(adapt_tmpl.and_then(|t| getter(t)))
}
