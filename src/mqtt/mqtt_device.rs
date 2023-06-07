use color_eyre::Result;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{protocols::mqtt::MqttClient, settings::Settings};

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Capabilities {
    /// XY color space (0.0 - 1.0)
    pub xy: bool,

    /// Color temperature (2000 - 6500)
    pub ct: Option<std::ops::Range<u16>>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Xy {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Ct {
    pub ct: u16,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DeviceColor {
    Xy(Xy),
    Ct(Ct),
}

#[derive(Builder, Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[builder(setter(into, strip_option), default)]
pub struct MqttDevice {
    pub id: String,
    pub name: String,
    pub power: Option<bool>,
    pub brightness: Option<f32>,
    pub color: Option<DeviceColor>,
    pub transition_ms: Option<f32>,
    pub sensor_value: Option<String>,
    pub capabilities: Option<Capabilities>,

    #[serde(skip_serializing, skip_deserializing)]
    pub updated: Option<String>,
}

pub async fn publish_mqtt_device(
    mqtt_client: &MqttClient,
    settings: &Settings,
    mqtt_device: &MqttDevice,
) -> Result<()> {
    let topic_template = if mqtt_device.sensor_value.is_some() {
        &settings.mqtt.sensor_topic
    } else {
        &settings.mqtt.light_topic
    };

    let topic = topic_template.replace("{id}", &mqtt_device.id);

    let json = serde_json::to_string(&mqtt_device)?;

    mqtt_client
        .client
        .publish(topic, rumqttc::QoS::AtLeastOnce, true, json)
        .await?;

    Ok(())
}
