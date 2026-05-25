use std::collections::HashSet;

#[derive(Clone, Default)]
pub enum SelectType {
    Modern,
    #[default]
    None,
    Raw,
}

#[derive(Clone, Debug, Default)]
pub enum Quality {
    Best,
    #[default]
    None,
    Worst,
}

#[derive(Clone, Debug, Default)]
pub struct Preferences {
    pub all: bool,
    pub skip: bool,
    pub languages: HashSet<String>,
    pub resolutions: HashSet<(u16, u16)>,
    pub quality: Quality,
}

#[derive(Clone, Debug, Default)]
pub struct SelectFilters {
    pub vid: Preferences,
    pub aud: Preferences,
    pub sub: Preferences,
    pub indices: HashSet<usize>,
    pub simple: bool,
}

impl SelectFilters {
    pub fn new(s: &str) -> Self {
        let mut filters = Self::default();

        // Simple format (solo): "1"
        if let Some(solo) = s
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|x| x.checked_sub(1))
        {
            filters.indices.insert(solo);
            filters.simple = true;
            return filters;
        }

        // Simple format (multi): "1,2,3"
        if s.contains(',') && !s.contains([':', 'v', 'a', 's', '=']) {
            filters.indices = s
                .split(',')
                .filter_map(|x| x.trim().parse::<usize>().ok())
                .filter_map(|x| x.checked_sub(1))
                .collect();
            filters.simple = true;
            return filters;
        }

        // Complex format: "v=best:a=en:s=skip"
        for stream in s.split_terminator(':') {
            let Some((code, queries)) = stream.split_once('=') else {
                continue;
            };

            for query in queries.split_terminator(',').map(|x| x.trim()) {
                if let Some(idx) = query.parse::<usize>().ok().and_then(|x| x.checked_sub(1)) {
                    filters.indices.insert(idx);
                    continue;
                }

                match code {
                    "v" => Self::parse_vid_query(query, &mut filters.vid),
                    "a" => Self::parse_lang_query(query, &mut filters.aud),
                    "s" => Self::parse_lang_query(query, &mut filters.sub),
                    _ => (),
                }
            }
        }

        filters
    }
}

impl SelectFilters {
    const RESOLUTIONS: &[(&str, (u16, u16))] = &[
        ("144p", (256, 144)),
        ("240p", (426, 240)),
        ("360p", (640, 360)),
        ("480p", (854, 480)),
        ("720p", (1280, 720)),
        ("hd", (1280, 720)),
        ("1080p", (1920, 1080)),
        ("fhd", (1920, 1080)),
        ("2k", (2048, 1080)),
        ("1440p", (2560, 1440)),
        ("qhd", (2560, 1440)),
        ("4k", (3840, 2160)),
        ("8k", (7680, 4320)),
    ];

    fn parse_vid_query(query: &str, prefs: &mut Preferences) {
        match query {
            "all" => prefs.all = true,
            "skip" => prefs.skip = true,
            "best" | "high" | "max" => prefs.quality = Quality::Best,
            "low" | "min" | "worst" => prefs.quality = Quality::Worst,
            q if q.contains('x') => {
                if let Some((w, h)) = q.split_once('x')
                    && let (Ok(w), Ok(h)) = (w.parse(), h.parse())
                {
                    prefs.resolutions.insert((w, h));
                }
            }
            q => {
                if let Some(&(_, res)) = Self::RESOLUTIONS.iter().find(|(name, _)| *name == q) {
                    prefs.resolutions.insert(res);
                }
            }
        }
    }

    fn parse_lang_query(query: &str, prefs: &mut Preferences) {
        match query {
            "all" => prefs.all = true,
            "skip" => prefs.skip = true,
            "best" | "high" | "max" => prefs.quality = Quality::Best,
            "low" | "min" | "worst" => prefs.quality = Quality::Worst,
            lang => {
                prefs.languages.insert(lang.to_owned());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_solo_index() {
        let filters = SelectFilters::new("1");
        assert!(filters.simple);
        assert_eq!(filters.indices.len(), 1);
        assert!(filters.indices.contains(&0));

        let filters = SelectFilters::new(" 10 ");
        assert!(filters.simple);
        assert_eq!(filters.indices.len(), 1);
        assert!(filters.indices.contains(&9));
    }

    #[test]
    fn test_simple_multi_index() {
        let filters = SelectFilters::new("1,2,3");
        assert!(filters.simple);
        assert_eq!(filters.indices.len(), 3);
        assert!(filters.indices.contains(&0));
        assert!(filters.indices.contains(&1));
        assert!(filters.indices.contains(&2));
    }

    #[test]
    fn test_complex_indices() {
        let filters = SelectFilters::new("v=1:a=2:s=3");
        assert!(!filters.simple);
        assert_eq!(filters.indices.len(), 3);
        assert!(filters.indices.contains(&0));
        assert!(filters.indices.contains(&1));
        assert!(filters.indices.contains(&2));
    }

    #[test]
    fn test_complex_filters() {
        let filters = SelectFilters::new("v=best,1080p,1920x1080:a=en,skip:s=all");
        assert!(!filters.simple);

        assert!(matches!(filters.vid.quality, Quality::Best));
        assert!(filters.vid.resolutions.contains(&(1920, 1080)));
        assert!(!filters.vid.all);
        assert!(!filters.vid.skip);

        assert!(filters.aud.skip);
        assert!(filters.aud.languages.contains("en"));
        assert!(!filters.aud.all);
        assert!(matches!(filters.aud.quality, Quality::None));

        assert!(filters.sub.all);
        assert!(!filters.sub.skip);
        assert!(filters.sub.languages.is_empty());
    }

    #[test]
    fn test_complex_resolutions() {
        let filters = SelectFilters::new("v=720p,4k,qhd,hd,360p");
        let res = &filters.vid.resolutions;
        assert!(res.contains(&(1280, 720)));
        assert!(res.contains(&(3840, 2160)));
        assert!(res.contains(&(2560, 1440)));
        assert!(res.contains(&(640, 360)));
    }

    #[test]
    fn test_complex_quality() {
        let filters = SelectFilters::new("v=high,worst");
        assert!(matches!(filters.vid.quality, Quality::Worst));

        let filters = SelectFilters::new("a=low,best");
        assert!(matches!(filters.aud.quality, Quality::Best));
    }
}
