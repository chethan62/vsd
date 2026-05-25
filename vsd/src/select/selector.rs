use crate::{
    error::Result,
    playlist::{MediaPlaylist, MediaType},
    select::parser::{Preferences, Quality, SelectFilters, SelectType},
};
use colored::Colorize;
use log::info;
use requestty::{Question, question::Choice};
use std::{
    collections::HashSet,
    io::{self, Write},
};

pub struct StreamSelector<'a> {
    indices: HashSet<usize>,
    streams: &'a [MediaPlaylist],
}

impl<'a> StreamSelector<'a> {
    pub fn new(streams: &'a [MediaPlaylist]) -> Self {
        Self {
            indices: HashSet::new(),
            streams,
        }
    }

    pub fn select(
        &mut self,
        filters: &SelectFilters,
        select_type: SelectType,
    ) -> Result<HashSet<usize>> {
        if filters.simple {
            self.indices = filters.indices.clone();
        } else {
            self.select_vid_streams(filters);
            self.select_lang_streams(MediaType::Audio, &filters.aud, &filters.indices);
            self.select_lang_streams(MediaType::Subtitles, &filters.sub, &filters.indices);

            for (i, s) in self.streams.iter().enumerate() {
                if s.media_type == MediaType::Undefined {
                    self.indices.insert(i);
                }
            }
        }

        match select_type {
            SelectType::Modern => self.interact_modern(),
            SelectType::Raw => self.interact_raw(),
            SelectType::None => {
                for (i, stream) in self.streams.iter().enumerate() {
                    let selected = self.indices.contains(&i);
                    info!(
                        "Stream [{}] {}",
                        stream.media_type.to_string().yellow(),
                        if selected {
                            stream.to_string().cyan()
                        } else {
                            stream.to_string().dimmed()
                        }
                    );
                }
                Ok(self.indices.clone())
            }
        }
    }

    fn select_vid_streams(&mut self, filters: &SelectFilters) {
        let data = self
            .streams
            .iter()
            .enumerate()
            .filter(|(_, s)| s.media_type == MediaType::Video)
            .map(|(i, s)| (i, s.resolution))
            .collect::<Vec<_>>();

        if filters.vid.all {
            for (i, _) in &data {
                self.indices.insert(*i);
            }
            return;
        }

        let mut indices = HashSet::new();

        for (i, _) in &data {
            if filters.indices.contains(i) {
                indices.insert(*i);
            }
        }

        match &filters.vid.quality {
            Quality::Best => {
                if let Some((i, _)) = data.first() {
                    indices.insert(*i);
                }
            }
            Quality::None => (),
            Quality::Worst => {
                if let Some((i, _)) = data.last() {
                    indices.insert(*i);
                }
            }
        }

        for (i, resolution) in &data {
            if let Some((w, h)) = resolution
                && filters.vid.resolutions.contains(&(*w as u16, *h as u16))
            {
                indices.insert(*i);
            }
        }

        self.apply_selection(&data, indices, filters.vid.skip);
    }

    fn select_lang_streams(
        &mut self,
        media_type: MediaType,
        prefs: &Preferences,
        stream_indices: &HashSet<usize>,
    ) {
        let data = self
            .streams
            .iter()
            .enumerate()
            .filter(|(_, s)| s.media_type == media_type)
            .map(|(i, s)| (i, s.language.clone()))
            .collect::<Vec<_>>();

        if prefs.all {
            for (i, _) in &data {
                self.indices.insert(*i);
            }
            return;
        }

        let mut indices = HashSet::new();

        for (i, _) in &data {
            if stream_indices.contains(i) {
                indices.insert(*i);
            }
        }

        let mut remaining = prefs.languages.clone();

        // Exact language match
        for (i, lang) in &data {
            if let Some(lang) = lang
                && remaining.remove(lang)
            {
                indices.insert(*i);
            }
        }

        // Similar language match (2-char prefix)
        for (i, lang) in &data {
            if let Some(lang) = lang {
                let code = lang.to_lowercase();
                let code = code.get(0..2);

                if let Some(matched) = remaining
                    .iter()
                    .find(|x| x.to_lowercase().get(0..2) == code)
                    .cloned()
                {
                    remaining.remove(&matched);
                    indices.insert(*i);
                }
            }
        }

        self.apply_selection(&data, indices, prefs.skip);
    }

    fn apply_selection<T>(&mut self, data: &[(usize, T)], mut indices: HashSet<usize>, skip: bool) {
        if skip && !indices.is_empty() {
            for (i, _) in data {
                if !indices.contains(i) {
                    self.indices.insert(*i);
                }
            }
        } else if !skip {
            if indices.is_empty()
                && let Some((i, _)) = data.first()
            {
                indices.insert(*i);
            }
            self.indices.extend(indices);
        }
    }

    fn choices(&self) -> (Vec<Choice<(String, bool)>>, Vec<Option<usize>>) {
        let mut choices = Vec::new();
        let mut choice_to_stream = Vec::new();

        for (media_type, header) in [
            (MediaType::Video, "─────── Video Streams ────────"),
            (MediaType::Audio, "─────── Audio Streams ────────"),
            (MediaType::Subtitles, "────── Subtitle Streams ──────"),
            (MediaType::Undefined, "───── Undefined Streams ──────"),
        ] {
            let streams = self
                .streams
                .iter()
                .enumerate()
                .filter(|(_, s)| s.media_type == media_type)
                .collect::<Vec<_>>();

            if streams.is_empty() {
                continue;
            }

            choices.push(Choice::Separator(header.to_owned()));
            choice_to_stream.push(None);

            for (i, stream) in streams {
                choices.push(Choice::Choice((
                    stream.to_string(),
                    self.indices.contains(&i),
                )));
                choice_to_stream.push(Some(i));
            }
        }

        (choices, choice_to_stream)
    }

    fn interact_modern(&self) -> Result<HashSet<usize>> {
        let (choices, choice_to_stream) = self.choices();
        let question = Question::multi_select("streams")
            .message("Select streams to download")
            .choices_with_default(choices)
            .transform(|choices, _, backend| {
                let summary = choices
                    .iter()
                    .map(|x| {
                        x.text
                            .split('|')
                            .map(|s| s.trim())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                backend.write_styled(&requestty::prompt::style::Stylize::cyan(&summary))
            })
            .build();
        let answer = requestty::prompt_one(question)?;
        let selected = answer
            .as_list_items()
            .unwrap()
            .iter()
            .filter_map(|item| choice_to_stream[item.index])
            .collect();
        Ok(selected)
    }

    fn interact_raw(&self) -> Result<HashSet<usize>> {
        let (choices, choice_to_stream) = self.choices();
        let stream_order = choice_to_stream
            .iter()
            .filter_map(|x| *x)
            .collect::<Vec<_>>();

        info!("Select streams to download:");

        let mut choice_num = 1_usize;
        let mut defaults = HashSet::new();

        for choice in choices {
            match choice {
                Choice::Separator(header) => info!("{}", header.replace('─', "-").cyan()),
                Choice::Choice((choice, selected)) => {
                    if selected {
                        defaults.insert(choice_num);
                    }
                    info!(
                        "{:>2}) [{}] {}",
                        choice_num,
                        if selected { "x".green() } else { " ".normal() },
                        choice
                    );
                    choice_num += 1;
                }
                _ => (),
            }
        }

        info!("{}", "------------------------------".cyan());
        print!("Press enter to proceed with defaults.\nOr select streams (1, 2, etc.): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        info!("{}", "------------------------------".cyan());

        let user_choices = if input.trim().is_empty() {
            defaults
        } else {
            input
                .trim()
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        };

        let selected = user_choices
            .into_iter()
            .filter_map(|n| stream_order.get(n.checked_sub(1)?).copied())
            .collect::<HashSet<_>>();

        for &i in &selected {
            if let Some(stream) = self.streams.get(i) {
                info!(
                    "Stream [{}] {}",
                    stream.media_type.to_string().yellow(),
                    stream.to_string().cyan()
                );
            }
        }

        info!("{}", "------------------------------".cyan());
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vid(width: u64, height: u64) -> MediaPlaylist {
        MediaPlaylist {
            media_type: MediaType::Video,
            resolution: Some((width, height)),
            ..Default::default()
        }
    }

    fn aud(language: &str) -> MediaPlaylist {
        MediaPlaylist {
            media_type: MediaType::Audio,
            language: Some(language.to_owned()),
            ..Default::default()
        }
    }

    fn sub(language: &str) -> MediaPlaylist {
        MediaPlaylist {
            media_type: MediaType::Subtitles,
            language: Some(language.to_owned()),
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
    fn test_simple() {
        let streams = vec![vid(1920, 1080), aud("en"), sub("es")];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("1,3"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&0));
        assert!(selected.contains(&2));
    }

    #[test]
    fn test_video_res() {
        let streams = vec![vid(1920, 1080), vid(1280, 720), vid(640, 360)];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("v=720p"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&1));
    }

    #[test]
    fn test_video_all() {
        let streams = vec![vid(1920, 1080), vid(1280, 720)];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("v=all"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&0));
        assert!(selected.contains(&1));
    }

    #[test]
    fn test_video_skip() {
        let streams = vec![vid(1920, 1080), vid(1280, 720), vid(640, 360)];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("v=skip,720p"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&0));
        assert!(selected.contains(&2));
    }

    #[test]
    fn test_video_fallback() {
        let streams = vec![vid(1920, 1080), vid(1280, 720)];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new(""), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&0));
    }

    #[test]
    fn test_audio_exact_lang() {
        let streams = vec![aud("en"), aud("fr"), aud("es")];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("a=fr"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&1));
    }

    #[test]
    fn test_audio_similar_lang() {
        let streams = vec![aud("fr-FR"), aud("en-US")];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("a=en"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&1));
    }

    #[test]
    fn test_audio_all() {
        let streams = vec![aud("en"), aud("fr")];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("a=all"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&0));
        assert!(selected.contains(&1));
    }

    #[test]
    fn test_audio_skip() {
        let streams = vec![aud("en"), aud("fr"), aud("es")];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new("a=skip,fr"), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&0));
        assert!(selected.contains(&2));
    }

    #[test]
    fn test_audio_fallback() {
        let streams = vec![aud("en"), aud("fr")];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new(""), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&0));
    }

    #[test]
    fn test_und_streams() {
        let streams = vec![vid(1920, 1080), und()];
        let selected = StreamSelector::new(&streams)
            .select(&SelectFilters::new(""), SelectType::None)
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&0));
        assert!(selected.contains(&1));
    }
}
