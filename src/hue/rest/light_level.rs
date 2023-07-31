use color_eyre::Result;
use serde::Deserialize;

use crate::{
    protocols::https::{mk_get_request, HyperHttpsClient},
    settings::Settings,
};

use super::common::Owner;

#[derive(Deserialize, Debug, Clone)]
pub struct LightLevelEventData {
    pub light_level: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LightLevelData {
    pub id: String,
    pub id_v1: String,
    pub owner: Owner,
    pub enabled: bool,
    pub light_level: Option<LightLevelEventData>,
}

#[derive(Deserialize, Debug, Clone)]
struct LightLevelResponse {
    data: Vec<LightLevelData>,
}

pub async fn get_hue_light_level(
    settings: &Settings,
    client: &HyperHttpsClient,
) -> Result<Vec<LightLevelData>> {
    let uri = format!(
        "https://{}/clip/v2/resource/light_level",
        settings.hue_bridge.addr
    )
    .parse()?;

    let response: LightLevelResponse = mk_get_request(client, settings, &uri).await?;

    Ok(response.data)
}
