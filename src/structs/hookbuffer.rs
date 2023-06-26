use serde::Deserialize;

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
