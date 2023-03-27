use color_eyre::Result;
use serde::Deserialize;

use crate::{
    protocols::https::{mk_get_request, HyperHttpsClient},
    settings::Settings,
};

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceService {
    pub rid: String,
    pub rtype: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceProductData {
    pub model_id: String,
    pub manufacturer_name: String,
    pub product_name: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceMetadata {
    pub name: String,
    pub archetype: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceData {
    pub id: String,
    pub id_v1: Option<String>,
    pub product_data: DeviceProductData,
    pub metadata: DeviceMetadata,
    pub services: Vec<DeviceService>,
}

#[derive(Deserialize, Debug, Clone)]
struct DeviceResponse {
    data: Vec<DeviceData>,
}

pub async fn get_hue_devices(
    settings: &Settings,
    client: &HyperHttpsClient,
) -> Result<Vec<DeviceData>> {
    let uri = format!(
        "https://{}/clip/v2/resource/device",
        settings.hue_bridge.addr
    )
    .parse()?;

    let response: DeviceResponse = mk_get_request(client, settings, &uri).await?;

    Ok(response.data)
}
