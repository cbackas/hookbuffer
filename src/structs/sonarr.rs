use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SonarrCustomFormat {
    pub id: Option<u64>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SonarrCustomFormatInfo {
    #[serde(rename = "customFormatScore")]
    pub custom_format_score: Option<u64>,
    #[serde(rename = "customFormats")]
    pub custom_formats: Option<Vec<SonarrCustomFormat>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SonarrEpisodeFile {
    #[serde(rename = "dateAdded")]
    pub date_added: Option<String>,
    pub id: Option<u64>,
    #[serde(rename = "mediaInfo")]
    pub media_info: Option<HashMap<String, Value>>,
    pub path: Option<String>,
    pub quality: String,
    #[serde(rename = "qualityVersion")]
    pub quality_version: Option<u64>,
    #[serde(rename = "relativePath")]
    pub relative_path: Option<String>,
    #[serde(rename = "releaseGroup")]
    pub release_group: Option<String>,
    #[serde(rename = "sceneName")]
    pub scene_name: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SonarrEpisode {
    #[serde(rename = "airDate")]
    pub air_date: Option<String>,
    #[serde(rename = "airDateUtc")]
    pub air_date_utc: Option<String>,
    #[serde(rename = "episodeNumber")]
    pub episode_number: u64,
    pub id: Option<u64>,
    pub overview: Option<String>,
    #[serde(rename = "seasonNumber")]
    pub season_number: u64,
    #[serde(rename = "seriesId")]
    pub series_id: u64,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SonarrRelease {
    #[serde(rename = "customFormatScore")]
    pub custom_format_score: Option<u64>,
    #[serde(rename = "customFormats")]
    pub custom_formats: Option<Vec<String>>,
    pub indexer: Option<String>,
    pub quality: Option<String>,
    #[serde(rename = "qualityVersion")]
    pub quality_version: Option<u64>,
    #[serde(rename = "releaseGroup")]
    pub release_group: Option<String>,
    #[serde(rename = "releaseTitle")]
    pub release_title: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SonarrSeries {
    pub id: Option<u64>,
    #[serde(rename = "imdbId")]
    pub imdb_id: Option<String>,
    pub path: Option<String>,
    pub title: String,
    #[serde(rename = "titleSlug")]
    pub title_slug: Option<String>,
    #[serde(rename = "tvMazeId")]
    pub tv_maze_id: Option<u64>,
    #[serde(rename = "tvdbId")]
    pub tvdb_id: Option<u64>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub year: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SonarrRequestBody {
    #[serde(rename = "applicationUrl")]
    pub application_url: Option<String>,
    #[serde(rename = "customFormatInfo")]
    pub custom_format_info: Option<SonarrCustomFormatInfo>,
    #[serde(rename = "downloadClient")]
    pub download_client: Option<String>,
    #[serde(rename = "downloadId")]
    pub download_id: Option<String>,
    pub episodes: Vec<SonarrEpisode>,
    #[serde(rename = "eventType")]
    pub event_type: Option<String>,
    #[serde(rename = "instanceName")]
    pub instance_name: Option<String>,
    pub release: Option<SonarrRelease>,
    pub series: SonarrSeries,
    #[serde(rename = "episodeFile")]
    pub episode_file: Option<SonarrEpisodeFile>,
    #[serde(rename = "isUpgrade")]
    pub is_upgrade: Option<bool>,
}
