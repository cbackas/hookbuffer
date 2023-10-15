use std::cmp::Ordering;

use reqwest::StatusCode;
use serde::Deserialize;

use super::sonarr::SonarrEventType;

#[derive(Deserialize, Debug, Clone)]
pub struct HBQuery {
    pub hb_output: HBOutput,
    pub hb_dest: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum HBOutput {
    Matrix,
    Discord,
}

#[derive(Debug)]
pub enum RequestError {
    Http(reqwest::Error),
    Other(StatusCode),
}

impl From<reqwest::Error> for RequestError {
    fn from(err: reqwest::Error) -> RequestError {
        RequestError::Http(err)
    }
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub struct SonarrGroupKey(pub u64, pub SonarrEventType, pub u64);

impl PartialOrd for SonarrGroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SonarrGroupKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0, &self.1, self.2).cmp(&(other.0, &other.1, other.2))
    }
}
