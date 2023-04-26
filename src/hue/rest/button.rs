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

#[derive(Deserialize, Debug, Clone)]
pub struct ButtonEventData {
    pub last_event: String,
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
