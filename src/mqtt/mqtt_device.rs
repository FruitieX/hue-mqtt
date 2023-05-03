use color_eyre::Result;
use derive_builder::Builder;
use palette::Hsv;
use serde::{Deserialize, Serialize};

use crate::{protocols::mqtt::MqttClient, settings::Settings};

#[derive(Builder, Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[builder(setter(into, strip_option), default)]
pub struct MqttDevice {
    pub id: String,
    pub name: String,
    pub power: Option<bool>,
    pub brightness: Option<f32>,
    pub cct: Option<f32>,
    pub color: Option<Hsv>,
    pub transition_ms: Option<f32>,
    pub sensor_value: Option<String>,

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
