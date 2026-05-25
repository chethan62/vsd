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

type ChoicesWithMapping = (Vec<Choice<(String, bool)>>, Vec<Option<usize>>);

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
        mut self,
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
                Ok(self.indices)
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

    fn build_choices(&self) -> ChoicesWithMapping {
        let mut choices = Vec::new();
        let mut choice_to_stream = Vec::new();

        for (media_type, header) in [
            (MediaType::Video, "─────── Video Streams ────────"),
            (MediaType::Audio, "─────── Audio Streams ────────"),
            (MediaType::Subtitles, "────── Subtitle Streams ──────"),
            (MediaType::Undefined, "───── Undefined Streams ──────"),
        ] {
            let type_streams = self
                .streams
                .iter()
                .enumerate()
                .filter(|(_, s)| s.media_type == media_type)
                .collect::<Vec<_>>();

            if type_streams.is_empty() {
                continue;
            }

            choices.push(requestty::Separator(header.into()));
            choice_to_stream.push(None);

            for (i, stream) in type_streams {
                choices.push(requestty::Choice((
                    stream.to_string(),
                    self.indices.contains(&i),
                )));
                choice_to_stream.push(Some(i));
            }
        }

        (choices, choice_to_stream)
    }

    fn interact_modern(self) -> Result<HashSet<usize>> {
        let (choices, choice_to_stream) = self.build_choices();

        let question = Question::multi_select("streams")
            .message("Select streams to download")
            .should_loop(false)
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

    fn interact_raw(self) -> Result<HashSet<usize>> {
        let (choices, choice_to_stream) = self.build_choices();
        let stream_order: Vec<usize> = choice_to_stream.iter().filter_map(|x| *x).collect();

        info!("Select streams to download:");

        let mut choice_num = 1_usize;
        let mut defaults = HashSet::new();

        for choice in &choices {
            match choice {
                requestty::Separator(header) => info!("{}", header.replace('─', "-").cyan()),
                requestty::Choice((msg, selected)) => {
                    if *selected {
                        defaults.insert(choice_num);
                    }
                    info!(
                        "{:>2}) [{}] {}",
                        choice_num,
                        if *selected { "x".green() } else { " ".normal() },
                        msg
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

        let selected: HashSet<usize> = user_choices
            .into_iter()
            .filter_map(|n| stream_order.get(n.checked_sub(1)?).copied())
            .collect();

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
