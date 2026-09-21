use super::sonarr::{SonarrEventType, SonarrRequestBody};

/// One de-duplicated episode row within a group, along with how many times it
/// was seen (Sonarr can fire the same episode more than once).
#[derive(Debug, Clone)]
pub struct EpisodeLine {
    pub season_number: u64,
    pub episode_number: u64,
    pub title: String,
    pub quality: String,
    pub count: u64,
}

impl EpisodeLine {
    /// Render a single episode line, e.g. `01x02 - Title [WEBDL-1080p] (2x)`.
    pub fn line(&self) -> String {
        match self.count {
            1 => format!(
                "{:02}x{:02} - {} [{}]",
                self.season_number, self.episode_number, self.title, self.quality
            ),
            _ => format!(
                "{:02}x{:02} - {} [{}] ({}x)",
                self.season_number, self.episode_number, self.title, self.quality, self.count
            ),
        }
    }
}

/// A presentation-agnostic summary of one grouped set of Sonarr events (same
/// series, event type, and season). Every forwarding target renders from this
/// so the grouping and line format stay identical regardless of destination.
#[derive(Debug, Clone)]
pub struct GroupSummary {
    pub event_type: SonarrEventType,
    pub series_title: String,
    pub season_number: u64,
    /// Headline verb + subject, e.g. `Imported: Series Season 02` or the
    /// single-episode form `Imported: Series - 02x01 - Episode Title`.
    pub headline: String,
    pub episodes: Vec<EpisodeLine>,
}

impl GroupSummary {
    /// The newline-joined episode list shared by all targets.
    pub fn description(&self) -> String {
        self.episodes
            .iter()
            .map(EpisodeLine::line)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Collapse one group of Sonarr events into a neutral [`GroupSummary`]: builds
/// the headline, de-duplicates episodes into counts, and sorts them.
pub fn summarize_group(sonarr_data: &[SonarrRequestBody]) -> GroupSummary {
    let event_type = sonarr_data[0].effective_event_type();
    let series_title = sonarr_data[0].series.title.clone();
    let season_number = sonarr_data[0].episodes[0].season_number;

    let verb = match event_type {
        SonarrEventType::Grab => "Grabbed",
        SonarrEventType::Download => "Imported",
        SonarrEventType::Upgrade => "Upgraded",
        SonarrEventType::Rename => "Renamed",
        _ => "Unknown",
    };
    let headline = match sonarr_data.len() {
        1 => format!(
            "{}: {} - {:02}x{:02} - {}",
            verb,
            series_title,
            season_number,
            sonarr_data[0].episodes[0].episode_number,
            sonarr_data[0].episodes[0].title
        ),
        _ => format!("{verb}: {series_title} Season {season_number:02}"),
    };

    let mut episodes: Vec<EpisodeLine> = sonarr_data
        .iter()
        .flat_map(|request| {
            let quality = request
                .episode_file
                .as_ref()
                .map(|episode_file| episode_file.quality.clone())
                .or_else(|| request.release.clone()?.quality)
                .unwrap_or_else(|| "None".to_string());
            request.episodes.iter().map(move |episode| {
                (
                    episode.season_number,
                    episode.episode_number,
                    episode.title.clone(),
                    quality.clone(),
                )
            })
        })
        .fold(
            Vec::new(),
            |mut acc: Vec<EpisodeLine>, (season, episode, title, quality)| {
                match acc
                    .iter()
                    .position(|line| line.season_number == season && line.episode_number == episode)
                {
                    Some(i) => acc[i].count += 1,
                    None => acc.push(EpisodeLine {
                        season_number: season,
                        episode_number: episode,
                        title,
                        quality,
                        count: 1,
                    }),
                };
                acc
            },
        );
    episodes.sort_by(|a, b| {
        a.season_number
            .cmp(&b.season_number)
            .then(a.episode_number.cmp(&b.episode_number))
            .then(a.title.cmp(&b.title))
            .then(a.quality.cmp(&b.quality))
    });

    GroupSummary {
        event_type,
        series_title,
        season_number,
        headline,
        episodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structs::sonarr::{SonarrEpisode, SonarrEpisodeFile, SonarrEventType, SonarrSeries};

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_str(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    fn create_episode_request(
        series_title: &str,
        episode_title: &str,
        season_number: u64,
        episode_number: u64,
        event_type: SonarrEventType,
    ) -> SonarrRequestBody {
        let series_id = hash_str(series_title);
        SonarrRequestBody {
            series: SonarrSeries {
                title: series_title.to_string(),
                id: Some(series_id),
                imdb_id: None,
                path: None,
                title_slug: None,
                tvdb_id: None,
                tv_maze_id: None,
                year: None,
                type_: None,
            },
            event_type: Some(event_type),
            episodes: vec![SonarrEpisode {
                episode_number,
                season_number,
                title: episode_title.to_string(),
                series_id,
                air_date: None,
                air_date_utc: None,
                id: None,
                overview: None,
            }],
            episode_file: Some(SonarrEpisodeFile {
                id: None,
                relative_path: None,
                path: None,
                quality: "Fake Quality".to_string(),
                quality_version: None,
                release_group: None,
                scene_name: None,
                size: None,
                date_added: None,
                media_info: None,
            }),
            release: None,
            is_upgrade: Some(event_type == SonarrEventType::Upgrade),
            application_url: None,
            download_client: None,
            download_id: None,
            custom_format_info: None,
            instance_name: None,
        }
    }

    #[test]
    fn multiple_episodes() {
        let requests = vec![
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 4",
                1,
                4,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 3",
                1,
                3,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 6",
                1,
                6,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 1",
                1,
                1,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 2",
                1,
                2,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 5",
                1,
                5,
                SonarrEventType::Grab,
            ),
        ];

        let summary = summarize_group(&requests);

        assert_eq!(summary.headline, "Grabbed: Fake Series 1 Season 01");
        assert_eq!(summary.series_title, "Fake Series 1");
        assert_eq!(
            summary.description(),
            "01x01 - Fake Episode 1 [Fake Quality]\n01x02 - Fake Episode 2 [Fake Quality]\n01x03 - Fake Episode 3 [Fake Quality]\n01x04 - Fake Episode 4 [Fake Quality]\n01x05 - Fake Episode 5 [Fake Quality]\n01x06 - Fake Episode 6 [Fake Quality]"
        );
    }

    #[test]
    fn single_episode() {
        let requests = vec![create_episode_request(
            "Fake Series",
            "Fake Episode 1",
            1,
            1,
            SonarrEventType::Grab,
        )];

        let summary = summarize_group(&requests);

        assert_eq!(
            summary.headline,
            "Grabbed: Fake Series - 01x01 - Fake Episode 1"
        );
        assert_eq!(summary.series_title, "Fake Series");
        assert_eq!(
            summary.description(),
            "01x01 - Fake Episode 1 [Fake Quality]"
        );
    }

    #[test]
    fn repeated_episodes() {
        let requests = vec![
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 4",
                1,
                4,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 4",
                1,
                4,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 4",
                1,
                4,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 1",
                1,
                1,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 1",
                1,
                1,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 1",
                1,
                1,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 6",
                1,
                6,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 6",
                1,
                6,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 6",
                1,
                6,
                SonarrEventType::Grab,
            ),
            create_episode_request(
                "Fake Series 1",
                "Fake Episode 6",
                1,
                6,
                SonarrEventType::Grab,
            ),
        ];

        let summary = summarize_group(&requests);

        assert_eq!(summary.headline, "Grabbed: Fake Series 1 Season 01");
        assert_eq!(
            summary.description(),
            "01x01 - Fake Episode 1 [Fake Quality] (3x)\n01x04 - Fake Episode 4 [Fake Quality] (3x)\n01x06 - Fake Episode 6 [Fake Quality] (4x)"
        );
    }

    // Upgrades must render identically regardless of which binary produced the
    // input: the worker keeps the raw `Download` + `isUpgrade` flag, while the
    // standalone rewrites the event type to `Upgrade` before grouping.

    #[test]
    fn upgrade_worker_representation() {
        let mut request = create_episode_request(
            "Fake Series",
            "Fake Episode 1",
            1,
            1,
            SonarrEventType::Download,
        );
        request.is_upgrade = Some(true);

        let summary = summarize_group(&[request]);

        assert_eq!(summary.event_type, SonarrEventType::Upgrade);
        assert_eq!(
            summary.headline,
            "Upgraded: Fake Series - 01x01 - Fake Episode 1"
        );
    }

    #[test]
    fn upgrade_standalone_representation() {
        let request = create_episode_request(
            "Fake Series",
            "Fake Episode 1",
            1,
            1,
            SonarrEventType::Upgrade,
        );

        let summary = summarize_group(&[request]);

        assert_eq!(summary.event_type, SonarrEventType::Upgrade);
        assert_eq!(
            summary.headline,
            "Upgraded: Fake Series - 01x01 - Fake Episode 1"
        );
    }
}
