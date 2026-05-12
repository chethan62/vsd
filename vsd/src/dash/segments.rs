use super::addressing::{
    merge_tmpl_field, process_segment_base, process_segment_list,
    process_segment_template_duration, process_segment_timeline, resolve_segment_template_init,
};
use super::{parse_locator, Template};
use crate::playlist::{
    Key, KeyMethod, Map, MediaPlaylist, Segment,
};
use anyhow::{Result, anyhow, bail};
use dash_mpd::{AdaptationSet, Representation, MPD};
use reqwest::{Client, Url};
use std::collections::HashMap;

/// Extract DRM encryption info from ContentProtection elements.
/// Scans representation-level first, falls back to adaptation-set-level.
fn extract_drm_info(
    repr_cp: &[dash_mpd::ContentProtection],
    adapt_cp: &[dash_mpd::ContentProtection],
) -> Option<Key> {
    let default_kid = repr_cp
        .iter()
        .find_map(|cp| cp.default_KID.clone())
        .or_else(|| adapt_cp.iter().find_map(|cp| cp.default_KID.clone()));

    let encryption_type = repr_cp
        .iter()
        .find(|cp| cp.value.is_some())
        .or_else(|| adapt_cp.iter().find(|cp| cp.value.is_some()))
        .map(|_| KeyMethod::Cenc)
        .unwrap_or(KeyMethod::None);

    match encryption_type {
        KeyMethod::None => None,
        method => Some(Key {
            default_kid: default_kid.map(|x| x.to_lowercase()),
            iv: None,
            key_format: None,
            method,
            uri: None,
        }),
    }
}

/// Prepare shared context: resolve base URL, period duration, and template vars.
fn prepare_context(
    base_url: &str,
    mpd: &MPD,
    period: &dash_mpd::Period,
    adaptation_set: &AdaptationSet,
    representation: &Representation,
) -> Result<(Url, f64, Template)> {
    let mut period_duration_secs = 0.0_f64;

    if let Some(duration) = &mpd.mediaPresentationDuration {
        period_duration_secs = duration.as_secs_f64();
    }

    if let Some(duration) = &period.duration {
        period_duration_secs = duration.as_secs_f64();
    }

    let mut base_url = base_url.parse::<Url>()?;

    if let Some(mpd_baseurl) = mpd.base_url.first().map(|x| x.base.as_ref()) {
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

    let rid = representation
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("missing @id on representation node."))?
        .to_owned();

    let mut template_vars = HashMap::from([("RepresentationID".to_owned(), rid)]);

    if let Some(bandwidth) = &representation.bandwidth {
        template_vars.insert("Bandwidth".to_owned(), bandwidth.to_string());
    }

    let template = Template::new(template_vars);

    Ok((base_url, period_duration_secs, template))
}

/// Try each addressing mode in priority order and return (segments, init_map).
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
) -> Result<(Vec<Segment>, Option<Map>)> {
    // (1.1) AdaptationSet > SegmentList
    if let Some(segment_list) = &adaptation_set.SegmentList {
        return process_segment_list(
            segment_list,
            base_url,
            template,
            !adaptation_set.BaseURL.is_empty(),
        );
    }

    // (1.2) Representation > SegmentList
    if let Some(segment_list) = &representation.SegmentList {
        return process_segment_list(
            segment_list,
            base_url,
            template,
            !representation.BaseURL.is_empty(),
        );
    }

    // (2, 3, 4) SegmentTemplate modes
    let repr_tmpl = representation.SegmentTemplate.as_ref();
    let adapt_tmpl = adaptation_set.SegmentTemplate.as_ref();

    if repr_tmpl.is_some() || adapt_tmpl.is_some() {
        let init_map = resolve_segment_template_init(repr_tmpl, adapt_tmpl, base_url, template)?;

        let tmpl_media = merge_tmpl_field(repr_tmpl, adapt_tmpl, |t| t.media.clone());
        let tmpl_timescale = merge_tmpl_field(repr_tmpl, adapt_tmpl, |t| t.timescale)
            .unwrap_or(1);
        let tmpl_start_number =
            merge_tmpl_field(repr_tmpl, adapt_tmpl, |t| t.startNumber).unwrap_or(1);

        // SegmentTimeline is a child element that also inherits from AdaptationSet
        let segment_timeline = repr_tmpl
            .and_then(|t| t.SegmentTimeline.as_ref())
            .or(adapt_tmpl.and_then(|t| t.SegmentTimeline.as_ref()));

        // (2) SegmentTemplate + SegmentTimeline
        if let Some(segment_timeline) = segment_timeline {
            let media = tmpl_media
                .as_deref()
                .ok_or_else(|| anyhow!("SegmentTimeline without a media attribute."))?;

            let segments = process_segment_timeline(
                segment_timeline,
                media,
                tmpl_start_number,
                tmpl_timescale,
                period_duration_secs,
                base_url,
                template,
            )?;

            return Ok((segments, init_map));
        }

        // (3, 4) SegmentTemplate@duration
        if let Some(media) = tmpl_media.as_deref() {
            let tmpl_duration = merge_tmpl_field(repr_tmpl, adapt_tmpl, |t| t.duration)
                .ok_or_else(|| {
                    anyhow!("Representation is missing SegmentTemplate@duration attribute.")
                })?;

            let segments = process_segment_template_duration(
                media,
                tmpl_start_number,
                tmpl_timescale,
                tmpl_duration,
                period_duration_secs,
                base_url,
                template,
            )?;

            return Ok((segments, init_map));
        }

        // SegmentTemplate present but no timeline or media — fall through
        return Ok((Vec::new(), init_map));
    }

    // (5) SegmentBase@indexRange
    if let Some(segment_base) = &representation.SegmentBase {
        return process_segment_base(segment_base, base_url, template, client, query).await;
    }

    // (6) Plain BaseURL
    if !representation.BaseURL.is_empty() {
        let segments = vec![Segment {
            duration: period_duration_secs as f32,
            uri: base_url.to_string(),
            ..Default::default()
        }];
        return Ok((segments, None));
    }

    Ok((Vec::new(), None))
}

/// Resolve the segments for a selected representation using the appropriate
/// DASH addressing mode. Iterates all periods and concatenates segments.
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
    let mut first_init_map: Option<Map> = None;
    let mut first_drm: Option<Key> = None;
    let mut resolved_base_url = None;

    for period in &mpd.periods {
        let Some(adaptation_set) = period.adaptations.get(loc_adaptation) else {
            continue;
        };
        let Some(representation) = adaptation_set.representations.get(loc_representation) else {
            continue;
        };

        let (base_url, period_duration_secs, mut template) =
            prepare_context(base_url, mpd, period, adaptation_set, representation)?;

        let (segments, init_map) = resolve_segments(
            client,
            query,
            adaptation_set,
            representation,
            &base_url,
            period_duration_secs,
            &mut template,
        )
        .await?;

        // Keep init map and DRM from the first period that has them
        if first_init_map.is_none() {
            first_init_map = init_map;
        }
        if first_drm.is_none() {
            first_drm = extract_drm_info(
                &representation.ContentProtection,
                &adaptation_set.ContentProtection,
            );
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
        first_segment.map = first_init_map;
    }

    if let Some(base_url) = resolved_base_url {
        stream.uri = base_url.to_string();
    }

    Ok(())
}
