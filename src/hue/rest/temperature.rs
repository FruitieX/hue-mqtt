use color_eyre::Result;
use serde::Deserialize;

use crate::{
    protocols::https::{mk_get_request, HyperHttpsClient},
    settings::Settings,
};

use super::common::Owner;

#[derive(Deserialize, Debug, Clone)]
pub struct TemperatureEventData {
    pub temperature: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TemperatureData {
    pub id: String,
    pub id_v1: String,
    pub owner: Owner,
    pub enabled: bool,
    pub temperature: Option<TemperatureEventData>,
}

#[derive(Deserialize, Debug, Clone)]
struct TemperatureResponse {
    data: Vec<TemperatureData>,
}

pub async fn get_hue_temperature(
    settings: &Settings,
    client: &HyperHttpsClient,
) -> Result<Vec<TemperatureData>> {
    let uri = format!(
        "https://{}/clip/v2/resource/temperature",
        settings.hue_bridge.addr
    )
    .parse()?;

    let response: TemperatureResponse = mk_get_request(client, settings, &uri).await?;

    Ok(response.data)
}
