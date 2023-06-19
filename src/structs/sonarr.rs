use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::collections::HashMap;

// Represents the series information.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Series {
    pub id: u64,
    pub imdb_id: String,
    pub path: String,
    pub title: String,
    pub title_slug: String,
    pub tv_maze_id: u64,
    pub tvdb_id: u64,
    pub type_: String,
    pub year: u64,
}

// Represents the episode information.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Episode {
    pub air_date: String,
    pub air_date_utc: String,
    pub episode_number: u64,
    pub id: u64,
    pub overview: String,
    pub season_number: u64,
    pub series_id: u64,
    pub title: String,
}

// Represents the custom format information.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomFormat {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomFormatInfo {
    pub custom_format_score: u64,
    pub custom_formats: Vec<CustomFormat>,
}

// Represents the release information.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Release {
    pub custom_format_score: u64,
    pub custom_formats: Vec<String>,
    pub indexer: String,
    pub quality: String,
    pub quality_version: u64,
    pub release_group: String,
    pub release_title: String,
    pub size: u64,
}

// Represents the episode file information.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpisodeFile {
    pub date_added: String,
    pub id: u64,
    pub media_info: HashMap<String, Value>,
    pub path: String,
    pub quality: String,
    pub quality_version: u64,
    pub relative_path: String,
    pub release_group: String,
    pub scene_name: String,
    pub size: u64,
}

// Represents the main request body.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestBody {
    pub application_url: String,
    pub custom_format_info: CustomFormatInfo,
    pub download_client: String,
    pub download_client_type: String,
    pub download_id: String,
    pub episodes: Vec<Episode>,
    pub event_type: String,
    pub instance_name: String,
    pub release: Release,
    pub series: Series,
    // Optional fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_file: Option<EpisodeFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_upgrade: Option<bool>,
}
