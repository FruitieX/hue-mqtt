use color_eyre::Result;
use serde::Deserialize;

use crate::{
    protocols::https::{mk_get_request, HyperHttpsClient},
    settings::Settings,
};

use super::common::Owner;

#[derive(Deserialize, Debug, Clone)]
pub struct Metadata {
    pub control_id: u32,
}

fn default_event() -> String {
    "short_release".to_string()
}

#[derive(Deserialize, Debug, Clone)]
pub struct ButtonReport {
    pub event: String,
    pub updated: String,
}

impl Default for ButtonReport {
    fn default() -> Self {
        Self {
            event: default_event(),
            updated: "1970-01-01T00:00:00.000Z".to_string(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ButtonEventData {
    #[serde(default = "default_event")]
    pub last_event: String,

    #[serde(default)]
    pub button_report: ButtonReport,
}

impl ButtonEventData {
    pub fn is_pressed(&self) -> bool {
        matches!(
            self.button_report.event.as_ref(),
            "initial_press" | "long_press" | "repeat"
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ButtonData {
    pub id: String,
    pub id_v1: String,
    pub owner: Owner,
    pub metadata: Metadata,
    pub button: Option<ButtonEventData>,
}

#[derive(Deserialize, Debug, Clone)]
struct ButtonResponse {
    data: Vec<ButtonData>,
}

pub async fn get_hue_buttons(
    settings: &Settings,
    client: &HyperHttpsClient,
) -> Result<Vec<ButtonData>> {
    let uri = format!(
        "https://{}/clip/v2/resource/button",
        settings.hue_bridge.addr
    )
    .parse()?;

    let response: ButtonResponse = mk_get_request(client, settings, &uri).await?;

    Ok(response.data)
}
