use color_eyre::Result;
use serde::Deserialize;

use crate::{
    protocols::https::{mk_get_request, HyperHttpsClient},
    settings::Settings,
};

use super::common::Owner;

#[derive(Deserialize, Debug, Clone)]
pub struct MotionEventData {
    pub motion: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MotionData {
    pub id: String,
    pub id_v1: String,
    pub owner: Owner,
    pub enabled: bool,
    pub motion: Option<MotionEventData>,
}

#[derive(Deserialize, Debug, Clone)]
struct MotionResponse {
    data: Vec<MotionData>,
}

pub async fn get_hue_motion(
    settings: &Settings,
    client: &HyperHttpsClient,
) -> Result<Vec<MotionData>> {
    let uri = format!(
        "https://{}/clip/v2/resource/motion",
        settings.hue_bridge.addr
    )
    .parse()?;

    let response: MotionResponse = mk_get_request(client, settings, &uri).await?;

    Ok(response.data)
}
