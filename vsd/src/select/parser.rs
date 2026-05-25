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
    pub stream_indices: HashSet<usize>,
    pub strict_indices: bool,
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
            filters.stream_indices.insert(solo);
            filters.strict_indices = true;
            return filters;
        }

        // Simple format (multi): "1,2,3"
        if s.contains(',') && !s.contains([':', 'v', 'a', 's', '=']) {
            filters.stream_indices = s
                .split(',')
                .filter_map(|x| x.trim().parse::<usize>().ok())
                .filter_map(|x| x.checked_sub(1))
                .collect();
            filters.strict_indices = true;
            return filters;
        }

        // Complex format: "v=best:a=en:s=skip"
        for stream in s.split_terminator(':') {
            let Some((code, queries)) = stream.split_once('=') else {
                continue;
            };

            for query in queries.split_terminator(',').map(|x| x.trim()) {
                if let Some(idx) = query.parse::<usize>().ok().and_then(|x| x.checked_sub(1)) {
                    filters.stream_indices.insert(idx);
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

impl Preferences {
    pub fn contains_exact_lang(&mut self, lang: &str) -> bool {
        if self.languages.contains(lang) {
            self.languages.remove(lang);
            return true;
        }
        false
    }

    pub fn contains_siml_lang(&mut self, lang: &str) -> bool {
        let code = lang.to_lowercase();
        let code = code.get(0..2);

        let lang = self
            .languages
            .iter()
            .find(|x| x.to_lowercase().get(0..2) == code)
            .cloned();

        if let Some(lang) = lang {
            self.languages.remove(&lang);
            return true;
        }
        false
    }
}
