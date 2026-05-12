mod addressing;
mod playlist;
mod segments;
mod template;

use crate::playlist::Range;
use template::Template;

pub(crate) use playlist::parse_as_master;
pub(crate) use segments::push_segments;

fn format_locator(adaptation_index: usize, representation_index: usize) -> String {
    format!("dash:{adaptation_index}.{representation_index}")
}

fn parse_locator(s: &str) -> Option<(usize, usize)> {
    let rest = s.strip_prefix("dash:")?;
    let (a, r) = rest.split_once('.')?;
    Some((a.parse().ok()?, r.parse().ok()?))
}

fn parse_channels(channels: &[dash_mpd::AudioChannelConfiguration]) -> Option<f32> {
    channels.first()?.value.as_ref()?.parse().ok()
}

fn parse_frame_rate(frame_rate: &str) -> Option<f32> {
    if let Some((x, y)) = frame_rate.split_once('/') {
        Some(x.parse::<f32>().ok()? / y.parse::<f32>().ok()?)
    } else {
        frame_rate.parse().ok()
    }
}

fn parse_range(range: &Option<String>) -> Option<Range> {
    let range = range.as_ref()?;
    let (x, y) = range.split_once('-')?;
    Some(Range {
        end: y.parse().ok()?,
        start: x.parse().ok()?,
    })
}
