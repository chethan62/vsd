use crate::{
    dash::{
        Template,
        addressing::{
            process_segment_base, process_segment_list, process_segment_template_duration,
            process_segment_template_init, process_segment_timeline,
        },
        parse_locator,
    },
    playlist::{Key, KeyMethod, MediaPlaylist, Segment},
};
use anyhow::{Result, anyhow, bail};
use dash_mpd::{AdaptationSet, MPD, Representation};
use reqwest::{Client, Url};

pub(crate) async fn push_segments(
    client: &Client,
    base_url: &str,
    query: &[(String, String)],
    mpd: &MPD,
    stream: &mut MediaPlaylist,
) -> Result<()> {
    let (loc_adaptation, loc_representation) = parse_locator(&stream.uri)
        .ok_or_else(|| anyhow!("invalid dash locator: {}", stream.uri))?;

    let mut all_segments: Vec<Segment> = Vec::new();
    let mut first_drm: Option<Key> = None;
    let mut resolved_base_url = None;

    for period in &mpd.periods {
        let Some(adaptation_set) = period.adaptations.get(loc_adaptation) else {
            continue;
        };
        let Some(representation) = adaptation_set.representations.get(loc_representation) else {
            continue;
        };

        // Resolve period duration (period-level overrides mpd-level)
        let period_duration_secs = period
            .duration
            .as_ref()
            .or(mpd.mediaPresentationDuration.as_ref())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        // Resolve base URL hierarchy: mpd → period → adaptation → representation
        let mut base_url = base_url.parse::<Url>()?;

        for url in [
            mpd.base_url.first().map(|x| x.base.as_ref()),
            period.BaseURL.first().map(|x| x.base.as_ref()),
            adaptation_set.BaseURL.first().map(|x| x.base.as_ref()),
            representation.BaseURL.first().map(|x| x.base.as_ref()),
        ]
        .into_iter()
        .flatten()
        {
            base_url = base_url.join(url)?;
        }

        // Build template variables
        let mut template = Template::new();
        template.insert(
            "RepresentationID",
            representation
                .id
                .clone()
                .ok_or_else(|| anyhow!("Missing @id on representation node."))?,
        );

        if let Some(bandwidth) = representation.bandwidth {
            template.insert("Bandwidth", bandwidth);
        }

        let segments = resolve_segments(
            client,
            query,
            adaptation_set,
            representation,
            &base_url,
            period_duration_secs,
            &mut template,
        )
        .await?;

        // Keep DRM from the first period that has it
        if first_drm.is_none() {
            let cp = representation
                .ContentProtection
                .iter()
                .chain(adaptation_set.ContentProtection.iter());

            let has_encryption = cp.clone().any(|c| c.value.is_some());

            if has_encryption {
                first_drm = Some(Key {
                    default_kid: cp
                        .clone()
                        .find_map(|c| c.default_KID.clone())
                        .map(|k| k.to_lowercase()),
                    iv: None,
                    key_format: None,
                    method: KeyMethod::Cenc,
                    uri: None,
                });
            }
        }

        if resolved_base_url.is_none() {
            resolved_base_url = Some(base_url);
        }

        all_segments.extend(segments);
    }

    if all_segments.is_empty() {
        bail!("no usable addressing mode identified for representation.");
    }

    stream.segments = all_segments;

    if let Some(first_segment) = stream.segments.get_mut(0) {
        first_segment.key = first_drm;
    }

    if let Some(base_url) = resolved_base_url {
        stream.uri = base_url.to_string();
    }

    Ok(())
}

/// Try each addressing mode in priority order and return segments.
/// Init maps are attached directly to the first segment.
///
/// Addressing modes (in order):
/// 1. AdaptationSet > SegmentList
/// 2. Representation > SegmentList
/// 3. SegmentTemplate + SegmentTimeline
/// 4. SegmentTemplate@duration
/// 5. SegmentBase@indexRange
/// 6. Plain BaseURL
async fn resolve_segments(
    client: &Client,
    query: &[(String, String)],
    adaptation_set: &AdaptationSet,
    representation: &Representation,
    base_url: &Url,
    period_duration_secs: f64,
    template: &mut Template,
) -> Result<Vec<Segment>> {
    // (1) AdaptationSet > SegmentList
    if let Some(segment_list) = &adaptation_set.SegmentList {
        return process_segment_list(
            segment_list,
            base_url,
            template,
            !adaptation_set.BaseURL.is_empty(),
        );
    }

    // (2) Representation > SegmentList
    if let Some(segment_list) = &representation.SegmentList {
        return process_segment_list(
            segment_list,
            base_url,
            template,
            !representation.BaseURL.is_empty(),
        );
    }

    // (3, 4, 5) SegmentTemplate modes
    let rt = representation.SegmentTemplate.as_ref();
    let at = adaptation_set.SegmentTemplate.as_ref();

    if rt.is_some() || at.is_some() {
        let init = process_segment_template_init(rt, at, base_url, template)?;

        let media = rt
            .and_then(|t| t.media.clone())
            .or(at.and_then(|t| t.media.clone()));
        let timescale = rt
            .and_then(|t| t.timescale)
            .or(at.and_then(|t| t.timescale))
            .unwrap_or(1);
        let start_number = rt
            .and_then(|t| t.startNumber)
            .or(at.and_then(|t| t.startNumber))
            .unwrap_or(1);

        let segment_timeline = rt
            .and_then(|t| t.SegmentTimeline.as_ref())
            .or(at.and_then(|t| t.SegmentTimeline.as_ref()));

        // (3) SegmentTemplate + SegmentTimeline
        if let Some(segment_timeline) = segment_timeline {
            let Some(media) = media.as_ref() else {
                bail!("SegmentTimeline without a media attribute.");
            };
            let mut segments = process_segment_timeline(
                segment_timeline,
                base_url,
                template,
                period_duration_secs,
                media,
                start_number,
                timescale,
            )?;

            if let Some(first) = segments.first_mut() {
                first.map = init;
            }

            return Ok(segments);
        }

        // (4) SegmentTemplate@duration
        if let Some(media) = media.as_deref() {
            let tmpl_duration = rt
                .and_then(|t| t.duration)
                .or(at.and_then(|t| t.duration))
                .ok_or_else(|| {
                    anyhow!("Representation is missing SegmentTemplate@duration attribute.")
                })?;

            let mut segments = process_segment_template_duration(
                media,
                start_number,
                timescale,
                tmpl_duration,
                period_duration_secs,
                base_url,
                template,
            )?;

            if let Some(first) = segments.first_mut() {
                first.map = init;
            }
            return Ok(segments);
        }

        // SegmentTemplate present but no timeline or media — fall through
        return Ok(Vec::new());
    }

    // (5) SegmentBase@indexRange
    if let Some(segment_base) = &representation.SegmentBase {
        return process_segment_base(segment_base, base_url, template, client, query).await;
    }

    // (6) Plain BaseURL
    if !representation.BaseURL.is_empty() {
        return Ok(vec![Segment {
            duration: period_duration_secs as f32,
            uri: base_url.to_string(),
            ..Default::default()
        }]);
    }

    Ok(Vec::new())
}
