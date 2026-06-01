use crate::{
    error::{Error, Result},
    playlist::{MediaPlaylist, MediaType},
};

#[derive(Clone, Debug, PartialEq)]
pub enum FormatExpr {
    Fallback(Vec<FormatExpr>),
    Merge(Vec<FormatExpr>),
    Single {
        base: BaseFormat,
        filters: Vec<Filter>,
    },
    Index(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum BaseFormat {
    BestVideo,
    BestAudio,
    WorstVideo,
    WorstAudio,
    Sub,
    All,
    AllVid,
    AllAud,
    AllSub,
    AllUnd,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub field: Field,
    pub op: FilterOp,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Width,
    Height,
    Resolution,
    Language,
    Tbr,
    Abr,
    Vbr,
    Fps,
    AudioChannels,
    Acodec,
    Vcodec,
    FormatId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterOp {
    Eq,
    Ne,
    Le,
    Ge,
    Lt,
    Gt,
    Contains,
    StartsWith,
    EndsWith,
}

impl FormatExpr {
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

    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(Error::FormatParse("expression cannot be empty".into()));
        }

        // Split by "/" for fallback chains (respecting brackets).
        let fallback_parts = Self::split_top_level(s, '/');

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
        let parts = Self::split_top_level(s, '+');

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
                    FormatExpr::Single {
                        base: BaseFormat::BestVideo,
                        filters: filters.clone(),
                    },
                    FormatExpr::Single {
                        base: BaseFormat::BestAudio,
                        filters,
                    },
                ]))
            }
            "w" | "worst" => {
                let filters = Self::parse_filters(filters_str)?;
                Ok(FormatExpr::Merge(vec![
                    FormatExpr::Single {
                        base: BaseFormat::WorstVideo,
                        filters: filters.clone(),
                    },
                    FormatExpr::Single {
                        base: BaseFormat::WorstAudio,
                        filters,
                    },
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

    /// Evaluate a format expression against a slice of media playlists.
    ///
    /// Returns the selected stream indices (0-based), in order.
    pub fn eval(&self, streams: &[MediaPlaylist]) -> Vec<usize> {
        match self {
            FormatExpr::Fallback(exprs) => {
                // Use strict evaluation: a branch is only accepted if every
                // sub-expression in its merge produced at least one result.
                for e in exprs {
                    if let Some(result) = e.eval_strict(streams) {
                        return result;
                    }
                }
                Vec::new()
            }
            FormatExpr::Merge(exprs) => {
                // Lenient: union all sub-expression results, silently
                // skipping any that match nothing.
                let mut merged = Vec::new();
                for e in exprs {
                    for idx in e.eval(streams) {
                        if !merged.contains(&idx) {
                            merged.push(idx);
                        }
                    }
                }
                merged
            }
            FormatExpr::Single { base, filters } => Self::eval_single(streams, base, filters),
            FormatExpr::Index(i) => {
                if *i < streams.len() {
                    vec![*i]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Strict evaluation: returns `None` if any part of the expression matches
    /// nothing.  Used by [`Fallback`] to decide whether a branch is fully
    /// satisfied before committing to it.
    fn eval_strict(&self, streams: &[MediaPlaylist]) -> Option<Vec<usize>> {
        match self {
            FormatExpr::Fallback(exprs) => {
                for e in exprs {
                    if let Some(result) = e.eval_strict(streams) {
                        return Some(result);
                    }
                }
                None
            }
            FormatExpr::Merge(exprs) => {
                let mut merged = Vec::new();
                for e in exprs {
                    let result = e.eval_strict(streams)?;
                    for idx in result {
                        if !merged.contains(&idx) {
                            merged.push(idx);
                        }
                    }
                }
                Some(merged)
            }
            FormatExpr::Single { base, filters } => {
                let result = Self::eval_single(streams, base, filters);
                if result.is_empty() {
                    None
                } else {
                    Some(result)
                }
            }
            FormatExpr::Index(i) => {
                if *i < streams.len() {
                    Some(vec![*i])
                } else {
                    None
                }
            }
        }
    }

    fn eval_single(streams: &[MediaPlaylist], base: &BaseFormat, filters: &[Filter]) -> Vec<usize> {
        let candidates: Vec<usize> = streams
            .iter()
            .enumerate()
            .filter(|(_, s)| Self::matches_base_type(s, base))
            .filter(|(_, s)| filters.iter().all(|f| Self::matches_filter(s, f)))
            .map(|(i, _)| i)
            .collect();

        match base {
            BaseFormat::BestVideo | BaseFormat::BestAudio | BaseFormat::Sub => {
                candidates.into_iter().take(1).collect()
            }
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
                Self::compare_numeric(w, &filter.op, &filter.value)
            }
            Field::Height => {
                let h = stream.resolution.map(|(_, h)| h as f64).unwrap_or(0.0);
                Self::compare_numeric(h, &filter.op, &filter.value)
            }
            Field::Tbr | Field::Abr | Field::Vbr => {
                let bw_kbps = stream.bandwidth.map(|b| b as f64 / 1000.0).unwrap_or(0.0);
                Self::compare_numeric(bw_kbps, &filter.op, &filter.value)
            }
            Field::Fps => {
                let fps = stream.frame_rate.map(|f| f as f64).unwrap_or(0.0);
                Self::compare_numeric(fps, &filter.op, &filter.value)
            }
            Field::AudioChannels => {
                let ch = stream.channels.map(|c| c as f64).unwrap_or(0.0);
                Self::compare_numeric(ch, &filter.op, &filter.value)
            }
            Field::Acodec | Field::Vcodec => {
                let codec = stream.codecs.as_deref().unwrap_or("");
                Self::compare_string(codec, &filter.op, &filter.value)
            }
            Field::Language => {
                let lang = stream.language.as_deref().unwrap_or("");
                Self::compare_string(lang, &filter.op, &filter.value)
            }
            Field::FormatId => Self::compare_string(&stream.id, &filter.op, &filter.value),
            Field::Resolution => {
                let res = stream
                    .resolution
                    .map(|(w, h)| format!("{}x{}", w, h))
                    .unwrap_or_default();
                Self::compare_string(&res, &filter.op, &filter.value)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keyword() {
        assert_eq!(
            FormatExpr::parse("bv").unwrap(),
            FormatExpr::Single {
                base: BaseFormat::BestVideo,
                filters: vec![],
            }
        );
    }

    #[test]
    fn parse_index() {
        assert_eq!(FormatExpr::parse("3").unwrap(), FormatExpr::Index(2));
    }

    #[test]
    fn parse_merge() {
        assert_eq!(
            FormatExpr::parse("bv+ba+s").unwrap(),
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
        if let FormatExpr::Fallback(parts) =
            FormatExpr::parse("bv[height=1080]+ba / bv[height=720]+ba").unwrap()
        {
            assert_eq!(parts.len(), 2);
        } else {
            panic!("expected fallback");
        }
    }

    #[test]
    fn parse_filter() {
        assert_eq!(
            FormatExpr::parse("ba[language=en]").unwrap(),
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
    fn parse_filters() {
        if let FormatExpr::Single { filters, .. } =
            FormatExpr::parse("bv[height<=720][vcodec^=avc1]").unwrap()
        {
            assert_eq!(filters.len(), 2);
        } else {
            panic!("expected single");
        }
    }

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
        assert_eq!(
            FormatExpr::parse("bv")
                .unwrap()
                .eval(&[vid(1920, 1080, 8000000), vid(1280, 720, 4500000)]),
            [0]
        );
    }

    #[test]
    fn eval_worst_video() {
        assert_eq!(
            FormatExpr::parse("wv")
                .unwrap()
                .eval(&[vid(1920, 1080, 8000000), vid(1280, 720, 4500000)]),
            [1]
        );
    }

    #[test]
    fn eval_merge() {
        assert_eq!(
            FormatExpr::parse("bv+ba+s").unwrap().eval(&[
                vid(1920, 1080, 8000000),
                aud("en", 512000),
                sub("en"),
            ]),
            [0, 1, 2]
        );
    }

    #[test]
    fn eval_index() {
        assert_eq!(
            FormatExpr::parse("1+3").unwrap().eval(&[
                vid(1920, 1080, 8000000),
                aud("en", 512000),
                sub("en"),
            ]),
            [0, 2]
        );
    }

    #[test]
    fn eval_filter_height() {
        assert_eq!(
            FormatExpr::parse("bv[height<=720]").unwrap().eval(&[
                vid(1920, 1080, 8000000),
                vid(1280, 720, 4500000),
                vid(640, 360, 1200000),
            ]),
            [1]
        );
    }

    #[test]
    fn eval_filter_language() {
        assert_eq!(
            FormatExpr::parse("ba[language=fr]").unwrap().eval(&[
                aud("en", 512000),
                aud("fr", 256000),
                aud("es", 128000)
            ]),
            [1]
        );
    }

    #[test]
    fn eval_filter_languages() {
        assert_eq!(
            FormatExpr::parse("allaud[language=en,fr]").unwrap().eval(&[
                aud("en", 512000),
                aud("fr", 256000),
                aud("es", 128000),
            ]),
            [0, 1]
        );
    }

    #[test]
    fn eval_all() {
        assert_eq!(
            FormatExpr::parse("all").unwrap().eval(&[
                vid(1920, 1080, 8000000),
                aud("en", 512000),
                sub("en")
            ]),
            [0, 1, 2]
        );
    }

    #[test]
    fn eval_allvid() {
        assert_eq!(
            FormatExpr::parse("allvid").unwrap().eval(&[
                vid(1920, 1080, 8000000),
                vid(1280, 720, 4500000),
                aud("en", 512000),
            ]),
            [0, 1]
        );
    }

    #[test]
    fn eval_fallback_first_success() {
        assert_eq!(
            FormatExpr::parse("bv[height=1080]+ba / bv[height=720]+ba")
                .unwrap()
                .eval(&[
                    vid(1920, 1080, 8000000),
                    vid(1280, 720, 4500000),
                    aud("en", 512000),
                ]),
            [0, 2]
        );
    }

    #[test]
    fn eval_fallback_first_fail() {
        assert_eq!(
            FormatExpr::parse("bv[height=1080]+ba / bv[height=720]+ba")
                .unwrap()
                .eval(&[vid(1280, 720, 4500000), aud("en", 512000)]),
            [0, 1]
        );
    }

    #[test]
    fn eval_default() {
        assert_eq!(
            FormatExpr::parse("b+s+allund").unwrap().eval(&[
                vid(1920, 1080, 8000000),
                vid(1280, 720, 4500000),
                aud("en", 512000),
                aud("fr", 256000),
                sub("en"),
                sub("fr"),
                und(),
            ]),
            [0, 2, 4, 6]
        );
    }

    #[test]
    fn eval_missing() {
        assert_eq!(
            FormatExpr::parse("b+s+allund")
                .unwrap()
                .eval(&[vid(1920, 1080, 8000000), aud("en", 512000)]),
            [0, 1]
        );
    }
}
