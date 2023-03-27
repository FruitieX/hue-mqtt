use derive_builder::Builder;
use palette::Hsv;
use serde::{Deserialize, Serialize};

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
}
