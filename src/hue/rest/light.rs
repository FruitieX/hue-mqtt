use color_eyre::Result;
use palette::{FromColor, Yxy};
use serde::{Deserialize, Serialize};

use crate::{
    protocols::{
        https::{mk_get_request, mk_put_request, HyperHttpsClient},
    },
    settings::Settings, mqtt_device::MqttDevice,
};

use super::common::Owner;

#[derive(Deserialize, Debug, Clone)]
pub struct LightMetadata {
    pub name: String,
    pub archetype: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OnData {
    pub on: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DimmingData {
    pub brightness: f32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ColorTemperatureData {
    pub mirek: Option<f32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ColorData {
    pub xy: XyData,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct XyData {
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LightData {
    pub id: String,
    pub id_v1: String,
    pub owner: Owner,
    pub metadata: LightMetadata,
    pub on: OnData,
    pub dimming: Option<DimmingData>,
    pub color: Option<ColorData>,
    pub color_temperature: Option<ColorTemperatureData>,
}

#[derive(Deserialize, Debug, Clone)]
struct LightResponse {
    data: Vec<LightData>,
}

pub async fn get_hue_lights(
    settings: &Settings,
    client: &HyperHttpsClient,
) -> Result<Vec<LightData>> {
    let uri = format!(
        "https://{}/clip/v2/resource/light",
        settings.hue_bridge.addr
    )
    .parse()?;

    let response: LightResponse = mk_get_request(client, settings, &uri).await?;

    Ok(response.data)
}

#[derive(Serialize, Debug, Clone)]
struct DynamicsData {
    duration: u32, // transition time measured in ms
}

#[derive(Serialize, Debug, Clone)]
struct LightRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    on: Option<OnData>,

    #[serde(skip_serializing_if = "Option::is_none")]
    dimming: Option<DimmingData>,

    #[serde(skip_serializing_if = "Option::is_none")]
    color_temperature: Option<ColorTemperatureData>,

    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<ColorData>,

    #[serde(skip_serializing_if = "Option::is_none")]
    dynamics: Option<DynamicsData>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PutError {
    pub description: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PutResourceIdentifier {
    pub rid: String,
    pub rtype: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PutResponse {
    pub errors: Vec<PutError>,
    pub data: Option<Vec<PutResourceIdentifier>>,
}

pub async fn put_hue_light(
    settings: &Settings,
    client: &HyperHttpsClient,
    mqtt_device: &MqttDevice,
) -> Result<PutResponse> {
    let uri = format!(
        "https://{}/clip/v2/resource/light/{}",
        settings.hue_bridge.addr, mqtt_device.id
    )
    .parse()?;

    let body = LightRequest {
        on: mqtt_device.power.map(|power| OnData { on: power }),
        dimming: mqtt_device.brightness.map(|brightness| DimmingData {
            brightness: brightness * 100.0,
        }),
        color_temperature: mqtt_device.cct.map(|cct| ColorTemperatureData {
            mirek: Some(1_000_000.0 / cct),
        }),
        color: mqtt_device.color.map(|color| -> ColorData {
            let yxy = Yxy::from_color(color);

            ColorData {
                xy: XyData { x: yxy.x, y: yxy.y },
            }
        }),
        dynamics: mqtt_device.transition_ms.map(|transition_ms| DynamicsData {
            duration: transition_ms as u32,
        }),
    };

    let response: PutResponse = mk_put_request(client, settings, &uri, &body).await?;

    Ok(response)
}
