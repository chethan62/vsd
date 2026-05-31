use crate::{
    error::{Error, Result},
    playlist::{MediaPlaylist, MediaType},
};

/// Parsed format expression AST.
///
/// Grammar:
///
/// ```text
/// expr        = merge ( "/" merge )*
/// merge       = single ( "+" single )*
/// single      = filter_expr | index
/// filter_expr = base filter*
/// base        = keyword (see BaseFormat)
/// index       = <integer>
/// filter      = "[" field op value "]"
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum FormatExpr {
    /// Fallback chain: try each in order, return first non-empty result.
    Fallback(Vec<FormatExpr>),
    /// Merge: combine selected streams from each sub-expression.
    Merge(Vec<FormatExpr>),
    /// A single format selector with optional filters.
    Single {
        base: BaseFormat,
        filters: Vec<Filter>,
    },
    /// A raw stream index (0-based internally, 1-based from user input).
    Index(usize),
}

/// Base format keyword determining which streams to consider.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseFormat {
    /// `bv` / `bestvideo` — best video stream.
    BestVideo,
    /// `ba` / `bestaudio` — best audio stream.
    BestAudio,
    /// `s` / `sub` — a subtitle stream (first available).
    Sub,
    /// `wv` / `worstvideo` — worst video stream.
    WorstVideo,
    /// `wa` / `worstaudio` — worst audio stream.
    WorstAudio,
    /// `all` — all streams of all types.
    All,
    /// `allvid` — all video streams.
    AllVid,
    /// `allaud` — all audio streams.
    AllAud,
    /// `allsub` — all subtitle streams.
    AllSub,
    /// `allund` — all undefined streams.
    AllUnd,
}

/// A filter applied to streams, e.g. `[height<=720]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub field: Field,
    pub op: FilterOp,
    pub value: String,
}

/// Filterable stream fields.
#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Width,
    Height,
    /// Total bitrate in kbps.
    Tbr,
    /// Audio bitrate in kbps.
    Abr,
    /// Video bitrate in kbps.
    Vbr,
    Fps,
    AudioChannels,
    Acodec,
    Vcodec,
    Language,
    FormatId,
    Resolution,
}

/// Filter comparison operators.
#[derive(Clone, Debug, PartialEq)]
pub enum FilterOp {
    /// `=` — equals (comma-separated values for OR match).
    Eq,
    /// `!=` — not equals (comma-separated values for NOR match).
    Ne,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `*=` — contains (string).
    Contains,
    /// `^=` — starts with.
    StartsWith,
    /// `$=` — ends with.
    EndsWith,
}

impl FormatExpr {
    /// Parse a format expression string into an AST.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// FormatExpr::parse("bv+ba+s")?;
    /// FormatExpr::parse("bv[height<=720]+ba[lang=en,fr]")?;
    /// FormatExpr::parse("bv[height=1080]+ba / bv[height=720]+ba")?;
    /// ```
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(Error::FormatParse("expression cannot be empty".into()));
        }

        // Split by "/" for fallback chains (respecting brackets).
        let fallback_parts = split_top_level(s, '/');

        if fallback_parts.len() == 1 {
            Self::parse_merge(fallback_parts[0].trim())
        } else {
            let mut exprs = Vec::new();
            for part in &fallback_parts {
                exprs.push(Self::parse_merge(part.trim())?);
            }
            Ok(FormatExpr::Fallback(exprs))
        }
    }

    fn parse_merge(s: &str) -> Result<Self> {
        let parts = split_top_level(s, '+');

        if parts.len() == 1 {
            Self::parse_single(parts[0].trim())
        } else {
            let mut exprs = Vec::new();
            for part in &parts {
                exprs.push(Self::parse_single(part.trim())?);
            }
            Ok(FormatExpr::Merge(exprs))
        }
    }

    fn parse_single(s: &str) -> Result<Self> {
        // Try parsing as an integer index first.
        if let Ok(idx) = s.parse::<usize>() {
            return if idx == 0 {
                Err(Error::FormatParse(
                    "stream index must be >= 1, got 0".into(),
                ))
            } else {
                Ok(FormatExpr::Index(idx - 1))
            };
        }

        // Expand shorthand keywords into composite expressions.
        let (base_str, filters_str) = if let Some(bracket_pos) = s.find('[') {
            (&s[..bracket_pos], &s[bracket_pos..])
        } else {
            (s, "")
        };

        match base_str.trim() {
            "b" | "best" => {
                let filters = Self::parse_filters(filters_str)?;
                Ok(FormatExpr::Merge(vec![
                    FormatExpr::Single { base: BaseFormat::BestVideo, filters: filters.clone() },
                    FormatExpr::Single { base: BaseFormat::BestAudio, filters },
                ]))
            }
            "w" | "worst" => {
                let filters = Self::parse_filters(filters_str)?;
                Ok(FormatExpr::Merge(vec![
                    FormatExpr::Single { base: BaseFormat::WorstVideo, filters: filters.clone() },
                    FormatExpr::Single { base: BaseFormat::WorstAudio, filters },
                ]))
            }
            _ => {
                let base = Self::parse_base(base_str.trim())?;
                let filters = Self::parse_filters(filters_str)?;
                Ok(FormatExpr::Single { base, filters })
            }
        }
    }

    fn parse_base(s: &str) -> Result<BaseFormat> {
        match s {
            "bv" | "bestvideo" => Ok(BaseFormat::BestVideo),
            "ba" | "bestaudio" => Ok(BaseFormat::BestAudio),
            "s" | "sub" => Ok(BaseFormat::Sub),
            "wv" | "worstvideo" => Ok(BaseFormat::WorstVideo),
            "wa" | "worstaudio" => Ok(BaseFormat::WorstAudio),
            "all" => Ok(BaseFormat::All),
            "allvid" => Ok(BaseFormat::AllVid),
            "allaud" => Ok(BaseFormat::AllAud),
            "allsub" => Ok(BaseFormat::AllSub),
            "allund" => Ok(BaseFormat::AllUnd),
            _ => return Err(Error::FormatParse(format!("unknown keyword '{}'", s))),
        }
    }

    fn parse_filters(s: &str) -> Result<Vec<Filter>> {
        if s.is_empty() {
            return Ok(Vec::new());
        }

        let mut filters = Vec::new();
        let mut remaining = s;

        while let Some(start) = remaining.find('[') {
            let end = remaining.find(']').ok_or_else(|| {
                Error::FormatParse(format!("unclosed bracket in '{}'", remaining))
            })?;
            let inner = &remaining[start + 1..end];
            filters.push(Self::parse_one_filter(inner)?);
            remaining = &remaining[end + 1..];
        }

        Ok(filters)
    }

    fn parse_one_filter(s: &str) -> Result<Filter> {
        // Try multi-char operators first, then single-char.
        let operators = [
            ("*=", FilterOp::Contains),
            ("^=", FilterOp::StartsWith),
            ("$=", FilterOp::EndsWith),
            ("<=", FilterOp::Le),
            (">=", FilterOp::Ge),
            ("!=", FilterOp::Ne),
            ("<", FilterOp::Lt),
            (">", FilterOp::Gt),
            ("=", FilterOp::Eq),
        ];

        for (op_str, op) in &operators {
            if let Some(pos) = s.find(op_str) {
                let field_str = s[..pos].trim();
                let value = s[pos + op_str.len()..].trim().to_owned();
                let field = Self::parse_field(field_str)?;
                return Ok(Filter {
                    field,
                    op: op.clone(),
                    value,
                });
            }
        }

        return Err(Error::FormatParse(format!("invalid filter '[{}]'", s)));
    }

    fn parse_field(s: &str) -> Result<Field> {
        match s {
            "width" => Ok(Field::Width),
            "height" => Ok(Field::Height),
            "tbr" => Ok(Field::Tbr),
            "abr" => Ok(Field::Abr),
            "vbr" => Ok(Field::Vbr),
            "fps" => Ok(Field::Fps),
            "audio_channels" => Ok(Field::AudioChannels),
            "acodec" => Ok(Field::Acodec),
            "vcodec" => Ok(Field::Vcodec),
            "language" => Ok(Field::Language),
            "format_id" => Ok(Field::FormatId),
            "resolution" => Ok(Field::Resolution),
            _ => return Err(Error::FormatParse(format!("unknown field '{}'", s))),
        }
    }
}

/// Split a string by a delimiter, but only at the top level (not inside brackets).
fn split_top_level(s: &str, delim: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            c if c == delim && depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => (),
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Evaluate a format expression against a slice of media playlists.
///
/// Returns the selected stream indices (0-based), in order.
pub fn select_formats(streams: &[MediaPlaylist], expr: &FormatExpr) -> Vec<usize> {
    eval(streams, expr)
}

fn eval(streams: &[MediaPlaylist], expr: &FormatExpr) -> Vec<usize> {
    match expr {
        FormatExpr::Fallback(exprs) => {
            for e in exprs {
                let result = eval(streams, e);
                if !result.is_empty() {
                    return result;
                }
            }
            Vec::new()
        }
        FormatExpr::Merge(exprs) => {
            let mut merged = Vec::new();
            for e in exprs {
                let result = eval(streams, e);
                if result.is_empty() {
                    return Vec::new();
                }
                for idx in result {
                    if !merged.contains(&idx) {
                        merged.push(idx);
                    }
                }
            }
            merged
        }
        FormatExpr::Single { base, filters } => eval_single(streams, base, filters),
        FormatExpr::Index(i) => {
            if *i < streams.len() {
                vec![*i]
            } else {
                Vec::new()
            }
        }
    }
}

fn eval_single(streams: &[MediaPlaylist], base: &BaseFormat, filters: &[Filter]) -> Vec<usize> {
    let candidates: Vec<usize> = streams
        .iter()
        .enumerate()
        .filter(|(_, s)| matches_base_type(s, base))
        .filter(|(_, s)| filters.iter().all(|f| matches_filter(s, f)))
        .map(|(i, _)| i)
        .collect();

    match base {
        BaseFormat::BestVideo
        | BaseFormat::BestAudio
        | BaseFormat::Sub => candidates.into_iter().take(1).collect(),
        BaseFormat::WorstVideo | BaseFormat::WorstAudio => {
            candidates.into_iter().last().into_iter().collect()
        }
        BaseFormat::All
        | BaseFormat::AllVid
        | BaseFormat::AllAud
        | BaseFormat::AllSub
        | BaseFormat::AllUnd => candidates,
    }
}

fn matches_base_type(stream: &MediaPlaylist, base: &BaseFormat) -> bool {
    match base {
        BaseFormat::All => true,
        BaseFormat::BestVideo | BaseFormat::WorstVideo | BaseFormat::AllVid => {
            stream.media_type == MediaType::Video
        }
        BaseFormat::BestAudio | BaseFormat::WorstAudio | BaseFormat::AllAud => {
            stream.media_type == MediaType::Audio
        }
        BaseFormat::Sub | BaseFormat::AllSub => stream.media_type == MediaType::Subtitles,
        BaseFormat::AllUnd => stream.media_type == MediaType::Undefined,
    }
}

fn matches_filter(stream: &MediaPlaylist, filter: &Filter) -> bool {
    match &filter.field {
        Field::Width => {
            let w = stream.resolution.map(|(w, _)| w as f64).unwrap_or(0.0);
            compare_numeric(w, &filter.op, &filter.value)
        }
        Field::Height => {
            let h = stream.resolution.map(|(_, h)| h as f64).unwrap_or(0.0);
            compare_numeric(h, &filter.op, &filter.value)
        }
        Field::Tbr | Field::Abr | Field::Vbr => {
            let bw_kbps = stream.bandwidth.map(|b| b as f64 / 1000.0).unwrap_or(0.0);
            compare_numeric(bw_kbps, &filter.op, &filter.value)
        }
        Field::Fps => {
            let fps = stream.frame_rate.map(|f| f as f64).unwrap_or(0.0);
            compare_numeric(fps, &filter.op, &filter.value)
        }
        Field::AudioChannels => {
            let ch = stream.channels.map(|c| c as f64).unwrap_or(0.0);
            compare_numeric(ch, &filter.op, &filter.value)
        }
        Field::Acodec | Field::Vcodec => {
            let codec = stream.codecs.as_deref().unwrap_or("");
            compare_string(codec, &filter.op, &filter.value)
        }
        Field::Language => {
            let lang = stream.language.as_deref().unwrap_or("");
            compare_string(lang, &filter.op, &filter.value)
        }
        Field::FormatId => {
            compare_string(&stream.id, &filter.op, &filter.value)
        }
        Field::Resolution => {
            let res = stream
                .resolution
                .map(|(w, h)| format!("{}x{}", w, h))
                .unwrap_or_default();
            compare_string(&res, &filter.op, &filter.value)
        }
    }
}

fn compare_numeric(actual: f64, op: &FilterOp, value_str: &str) -> bool {
    // Small epsilon to handle f32 -> f64 precision loss.
    const EPS: f64 = 0.001;

    // Support comma-separated values for Eq/Ne.
    match op {
        FilterOp::Eq => value_str
            .split(',')
            .filter_map(|v| v.trim().parse::<f64>().ok())
            .any(|v| (actual - v).abs() < EPS),
        FilterOp::Ne => value_str
            .split(',')
            .filter_map(|v| v.trim().parse::<f64>().ok())
            .all(|v| (actual - v).abs() >= EPS),
        _ => {
            let Ok(target) = value_str.trim().parse::<f64>() else {
                return false;
            };
            match op {
                FilterOp::Le => actual <= target + EPS,
                FilterOp::Ge => actual >= target - EPS,
                FilterOp::Lt => actual < target - EPS,
                FilterOp::Gt => actual > target + EPS,
                _ => false, // String ops don't apply to numeric.
            }
        }
    }
}

fn compare_string(actual: &str, op: &FilterOp, value_str: &str) -> bool {
    let actual_lower = actual.to_lowercase();

    match op {
        FilterOp::Eq => value_str.split(',').any(|v| {
            let v = v.trim().to_lowercase();
            actual_lower == v || actual_lower.starts_with(&format!("{}-", v))
        }),
        FilterOp::Ne => value_str.split(',').all(|v| {
            let v = v.trim().to_lowercase();
            actual_lower != v && !actual_lower.starts_with(&format!("{}-", v))
        }),
        FilterOp::Contains => {
            let v = value_str.to_lowercase();
            actual_lower.contains(&v)
        }
        FilterOp::StartsWith => {
            let v = value_str.to_lowercase();
            actual_lower.starts_with(&v)
        }
        FilterOp::EndsWith => {
            let v = value_str.to_lowercase();
            actual_lower.ends_with(&v)
        }
        // Numeric ops on strings: compare lexicographically.
        FilterOp::Le => actual_lower <= value_str.to_lowercase(),
        FilterOp::Ge => actual_lower >= value_str.to_lowercase(),
        FilterOp::Lt => actual_lower < value_str.to_lowercase(),
        FilterOp::Gt => actual_lower > value_str.to_lowercase(),
    }
}



// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parser Tests ─────────────────────────────────────────────────────

    #[test]
    fn parse_simple_keyword() {
        let expr = FormatExpr::parse("bv").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Single {
                base: BaseFormat::BestVideo,
                filters: vec![],
            }
        );
    }

    #[test]
    fn parse_aliases() {
        let expr = FormatExpr::parse("bestvideo").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Single {
                base: BaseFormat::BestVideo,
                filters: vec![],
            }
        );

        let expr = FormatExpr::parse("sub").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Single {
                base: BaseFormat::Sub,
                filters: vec![],
            }
        );
    }

    #[test]
    fn parse_index() {
        let expr = FormatExpr::parse("3").unwrap();
        assert_eq!(expr, FormatExpr::Index(2));
    }

    #[test]
    fn parse_index_zero_error() {
        assert!(FormatExpr::parse("0").is_err());
    }

    #[test]
    fn parse_merge() {
        let expr = FormatExpr::parse("bv+ba+s").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Merge(vec![
                FormatExpr::Single {
                    base: BaseFormat::BestVideo,
                    filters: vec![],
                },
                FormatExpr::Single {
                    base: BaseFormat::BestAudio,
                    filters: vec![],
                },
                FormatExpr::Single {
                    base: BaseFormat::Sub,
                    filters: vec![],
                },
            ])
        );
    }

    #[test]
    fn parse_fallback() {
        let expr = FormatExpr::parse("bv[height=1080]+ba / bv[height=720]+ba").unwrap();
        match expr {
            FormatExpr::Fallback(parts) => {
                assert_eq!(parts.len(), 2);
            }
            _ => panic!("Expected Fallback"),
        }
    }

    #[test]
    fn parse_filter_eq() {
        let expr = FormatExpr::parse("ba[language=en]").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Single {
                base: BaseFormat::BestAudio,
                filters: vec![Filter {
                    field: Field::Language,
                    op: FilterOp::Eq,
                    value: "en".to_owned(),
                }],
            }
        );
    }

    #[test]
    fn parse_filter_le() {
        let expr = FormatExpr::parse("bv[height<=720]").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Single {
                base: BaseFormat::BestVideo,
                filters: vec![Filter {
                    field: Field::Height,
                    op: FilterOp::Le,
                    value: "720".to_owned(),
                }],
            }
        );
    }

    #[test]
    fn parse_filter_contains() {
        let expr = FormatExpr::parse("bv[vcodec*=avc]").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Single {
                base: BaseFormat::BestVideo,
                filters: vec![Filter {
                    field: Field::Vcodec,
                    op: FilterOp::Contains,
                    value: "avc".to_owned(),
                }],
            }
        );
    }

    #[test]
    fn parse_multiple_filters() {
        let expr = FormatExpr::parse("bv[height<=720][vcodec^=avc1]").unwrap();
        match expr {
            FormatExpr::Single { filters, .. } => assert_eq!(filters.len(), 2),
            _ => panic!("Expected Single"),
        }
    }

    #[test]
    fn parse_comma_value() {
        let expr = FormatExpr::parse("allaud[language=en,fr,de]").unwrap();
        assert_eq!(
            expr,
            FormatExpr::Single {
                base: BaseFormat::AllAud,
                filters: vec![Filter {
                    field: Field::Language,
                    op: FilterOp::Eq,
                    value: "en,fr,de".to_owned(),
                }],
            }
        );
    }

    #[test]
    fn parse_unknown_keyword_error() {
        assert!(FormatExpr::parse("xyz").is_err());
    }

    #[test]
    fn parse_empty_error() {
        assert!(FormatExpr::parse("").is_err());
    }

    #[test]
    fn parse_unclosed_bracket_error() {
        assert!(FormatExpr::parse("bv[height<=720").is_err());
    }

    // ── Evaluator Tests ──────────────────────────────────────────────────

    fn vid(width: u64, height: u64, bw: u64) -> MediaPlaylist {
        MediaPlaylist {
            media_type: MediaType::Video,
            resolution: Some((width, height)),
            bandwidth: Some(bw),
            codecs: Some("avc1.640028".to_owned()),
            frame_rate: Some(30.0),
            ..Default::default()
        }
    }

    fn aud(lang: &str, bw: u64) -> MediaPlaylist {
        MediaPlaylist {
            media_type: MediaType::Audio,
            language: Some(lang.to_owned()),
            bandwidth: Some(bw),
            codecs: Some("mp4a.40.2".to_owned()),
            channels: Some(2.0),
            ..Default::default()
        }
    }

    fn sub(lang: &str) -> MediaPlaylist {
        MediaPlaylist {
            media_type: MediaType::Subtitles,
            language: Some(lang.to_owned()),
            ..Default::default()
        }
    }

    fn und() -> MediaPlaylist {
        MediaPlaylist {
            media_type: MediaType::Undefined,
            ..Default::default()
        }
    }

    #[test]
    fn eval_best_video() {
        let streams = vec![vid(1920, 1080, 8000000), vid(1280, 720, 4500000)];
        let expr = FormatExpr::parse("bv").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_worst_video() {
        let streams = vec![vid(1920, 1080, 8000000), vid(1280, 720, 4500000)];
        let expr = FormatExpr::parse("wv").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![1]);
    }

    #[test]
    fn eval_best_audio() {
        let streams = vec![aud("en", 512000), aud("fr", 256000)];
        let expr = FormatExpr::parse("ba").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_sub() {
        let streams = vec![sub("en"), sub("fr")];
        let expr = FormatExpr::parse("s").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_merge() {
        let streams = vec![vid(1920, 1080, 8000000), aud("en", 512000), sub("en")];
        let expr = FormatExpr::parse("bv+ba+s").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0, 1, 2]);
    }

    #[test]
    fn eval_index() {
        let streams = vec![vid(1920, 1080, 8000000), aud("en", 512000), sub("en")];
        let expr = FormatExpr::parse("1+3").unwrap();
        let selected = select_formats(&streams, &expr);
        // Index-only: no und auto-include.
        assert_eq!(selected, vec![0, 2]);
    }

    #[test]
    fn eval_filter_height_le() {
        let streams = vec![
            vid(1920, 1080, 8000000),
            vid(1280, 720, 4500000),
            vid(640, 360, 1200000),
        ];
        let expr = FormatExpr::parse("bv[height<=720]").unwrap();
        let selected = select_formats(&streams, &expr);
        // Best video with height <= 720 → first match is index 1 (720p).
        assert_eq!(selected, vec![1]);
    }

    #[test]
    fn eval_filter_lang_eq() {
        let streams = vec![aud("en", 512000), aud("fr", 256000), aud("es", 128000)];
        let expr = FormatExpr::parse("ba[language=fr]").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![1]);
    }

    #[test]
    fn eval_filter_lang_comma() {
        let streams = vec![aud("en", 512000), aud("fr", 256000), aud("es", 128000)];
        let expr = FormatExpr::parse("allaud[language=en,fr]").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0, 1]);
    }

    #[test]
    fn eval_filter_lang_ne() {
        let streams = vec![aud("en", 512000), aud("fr", 256000), aud("es", 128000)];
        let expr = FormatExpr::parse("allaud[language!=en]").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![1, 2]);
    }

    #[test]
    fn eval_filter_bandwidth_kbps() {
        let streams = vec![aud("en", 512000), aud("fr", 128000)];
        let expr = FormatExpr::parse("ba[tbr>=256]").unwrap();
        let selected = select_formats(&streams, &expr);
        // 512000 bps = 512 kbps >= 256 → match.
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_filter_codec_contains() {
        let streams = vec![
            vid(1920, 1080, 8000000),
            vid(1280, 720, 4500000),
        ];
        let expr = FormatExpr::parse("bv[vcodec*=avc]").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_filter_channels() {
        let mut s1 = aud("en", 512000);
        s1.channels = Some(5.1);
        let s2 = aud("en", 256000);
        let streams = vec![s1, s2];
        let expr = FormatExpr::parse("ba[audio_channels>=5.1]").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_all() {
        let streams = vec![vid(1920, 1080, 8000000), aud("en", 512000), sub("en")];
        let expr = FormatExpr::parse("all").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0, 1, 2]);
    }

    #[test]
    fn eval_allvid() {
        let streams = vec![
            vid(1920, 1080, 8000000),
            vid(1280, 720, 4500000),
            aud("en", 512000),
        ];
        let expr = FormatExpr::parse("allvid").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0, 1]);
    }

    #[test]
    fn eval_fallback_partial_merge_triggers_fallback() {
        // bv[height=1080]+ba: no 1080p stream → bv part is empty → entire merge is empty →
        // fallback to bv[height=720]+ba which matches.
        let streams = vec![vid(1280, 720, 4500000), aud("en", 512000)];
        let expr = FormatExpr::parse("bv[height=1080]+ba / bv[height=720]+ba").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0, 1]);
    }

    #[test]
    fn eval_fallback_first_branch_succeeds() {
        // First branch fully matches (1080p exists), so fallback is never tried.
        let streams = vec![
            vid(1920, 1080, 8000000),
            vid(1280, 720, 4500000),
            aud("en", 512000),
        ];
        let expr = FormatExpr::parse("bv[height=1080]+ba / bv[height=720]+ba").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0, 2]);
    }

    #[test]
    fn eval_allund() {
        let streams = vec![vid(1920, 1080, 8000000), und(), und()];
        let expr = FormatExpr::parse("bv+allund").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0, 1, 2]);
    }

    #[test]
    fn eval_und_not_included_without_allund() {
        let streams = vec![vid(1920, 1080, 8000000), und()];
        let expr = FormatExpr::parse("bv").unwrap();
        let selected = select_formats(&streams, &expr);
        // bv only selects video, und is NOT auto-included.
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_lang_prefix_match() {
        let streams = vec![aud("en-US", 512000), aud("fr-FR", 256000)];
        let expr = FormatExpr::parse("ba[language=en]").unwrap();
        let selected = select_formats(&streams, &expr);
        assert_eq!(selected, vec![0]);
    }

    #[test]
    fn eval_no_match_returns_empty() {
        let streams = vec![vid(1920, 1080, 8000000)];
        let expr = FormatExpr::parse("ba").unwrap();
        let selected = select_formats(&streams, &expr);
        assert!(selected.is_empty());
    }

    #[test]
    fn eval_best_expands() {
        let streams = vec![
            vid(1920, 1080, 8000000),
            vid(1280, 720, 4500000),
            aud("en", 512000),
            aud("fr", 256000),
            sub("en"),
        ];
        let expr = FormatExpr::parse("b").unwrap();
        let selected = select_formats(&streams, &expr);
        // b = bv+ba → best video (0) + best audio (2).
        assert_eq!(selected, vec![0, 2]);
    }

    #[test]
    fn eval_worst_expands() {
        let streams = vec![
            vid(1920, 1080, 8000000),
            vid(1280, 720, 4500000),
            aud("en", 512000),
            aud("fr", 256000),
        ];
        let expr = FormatExpr::parse("w").unwrap();
        let selected = select_formats(&streams, &expr);
        // w = wv+wa → worst video (1) + worst audio (3).
        assert_eq!(selected, vec![1, 3]);
    }

    #[test]
    fn eval_default_expression() {
        let streams = vec![
            vid(1920, 1080, 8000000),
            vid(1280, 720, 4500000),
            aud("en", 512000),
            aud("fr", 256000),
            sub("en"),
            sub("fr"),
            und(),
        ];
        let expr = FormatExpr::parse("b+s+allund").unwrap();
        let selected = select_formats(&streams, &expr);
        // b(bv+ba) + s + allund → best video (0) + best audio (2) + first sub (4) + und (6).
        assert_eq!(selected, vec![0, 2, 4, 6]);
    }
}
