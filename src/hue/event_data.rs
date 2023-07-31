use std::collections::HashMap;

use color_eyre::Result;
use eyre::eyre;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::mqtt::mqtt_device::{Ct, DeviceColor, MqttDevice, Xy};

use super::rest::{
    button::ButtonEventData,
    light::{ColorData, ColorTemperatureData, DimmingData, OnData},
};

#[derive(Deserialize, Debug, Clone)]
struct ButtonUpdateData {
    id: String,
    button: ButtonEventData,
}

#[derive(Deserialize, Debug, Clone)]
struct LightUpdateData {
    id: String,
    on: Option<OnData>,
    dimming: Option<DimmingData>,
    color: Option<ColorData>,
    color_temperature: Option<ColorTemperatureData>,
}

#[derive(Deserialize, Debug, Clone)]
struct MotionData {
    motion: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct MotionUpdateData {
    id: String,
    motion: MotionData,
}

#[derive(Deserialize, Debug, Clone)]
struct TemperatureData {
    temperature: f64,
}

#[derive(Deserialize, Debug, Clone)]
struct TemperatureUpdateData {
    id: String,
    temperature: TemperatureData,
}

#[derive(Deserialize, Debug, Clone)]
struct LightLevelData {
    light_level: f64,
}

#[derive(Deserialize, Debug, Clone)]
struct LightLevelUpdateData {
    id: String,
    light: LightLevelData,
}

#[derive(Deserialize, Debug, Clone)]
struct DevicePowerData {}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum UpdateData {
    Button(ButtonUpdateData),
    Light(LightUpdateData),
    Motion(MotionUpdateData),
    Temperature(TemperatureUpdateData),
    LightLevel(LightLevelUpdateData),

    // Ignored updates
    DevicePower,        // Battery level update
    GroupedLight,       // Light groups update
    ZigbeeConnectivity, // Connectivity issue update
}

impl UpdateData {
    /// Computes current device state from previous device state and an UpdateData
    /// containing one field that's changed.
    fn to_mqtt_device(&self, mqtt_devices: &HashMap<String, MqttDevice>) -> Option<MqttDevice> {
        match self {
            UpdateData::Button(button) => {
                let sensor_value = match button.button.last_event.as_str() {
                    "short_release" | "long_release" => Some(false),
                    "initial_press" => Some(true),
                    _ => None,
                };

                let mut mqtt_device = mqtt_devices.get(&button.id)?.clone();

                // Ignore already seen button reports
                if let Some(updated) = mqtt_device.updated.as_ref() {
                    if updated == &button.button.button_report.updated {
                        return None;
                    }
                }

                if let Some(sensor_value) = sensor_value {
                    mqtt_device.sensor_value = Some(sensor_value.to_string());
                    mqtt_device.updated = Some(button.button.button_report.updated.clone());

                    return Some(mqtt_device);
                }
            }
            UpdateData::Motion(motion) => {
                let mut mqtt_device = mqtt_devices.get(&motion.id)?.clone();

                mqtt_device.sensor_value = Some(motion.motion.motion.to_string());

                return Some(mqtt_device);
            }
            UpdateData::Temperature(temperature) => {
                let mut mqtt_device = mqtt_devices.get(&temperature.id)?.clone();

                mqtt_device.sensor_value = Some(temperature.temperature.temperature.to_string());

                return Some(mqtt_device);
            }
            UpdateData::LightLevel(light_level) => {
                let mut mqtt_device = mqtt_devices.get(&light_level.id)?.clone();

                mqtt_device.sensor_value = Some(light_level.light.light_level.to_string());

                return Some(mqtt_device);
            }
            UpdateData::Light(light) => {
                let mut mqtt_device = mqtt_devices.get(&light.id)?.clone();

                if let Some(color) = &light.color {
                    mqtt_device.color = Some(DeviceColor::Xy(Xy {
                        x: color.xy.x,
                        y: color.xy.y,
                    }));
                }

                if let Some(ColorTemperatureData {
                    mirek: Some(mirek), ..
                }) = light.color_temperature
                {
                    let ct = (1_000_000.0 / mirek) as u16;
                    mqtt_device.color = Some(DeviceColor::Ct(Ct { ct }));
                }

                if let Some(on) = &light.on {
                    mqtt_device.power = Some(on.on);
                }

                if let Some(dimming) = &light.dimming {
                    mqtt_device.brightness = Some(dimming.brightness / 100.0)
                }

                return Some(mqtt_device);
            }

            _ => {}
        };

        None
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateEvent {
    data: Vec<UpdateData>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HueEvent {
    Update(UpdateEvent),
}

pub async fn handle_incoming_hue_events(
    mqtt_devices: &RwLock<HashMap<String, MqttDevice>>,
    events: String,
    ignore_buttons: bool,
) -> Result<Vec<MqttDevice>> {
    let serde_json_value: serde_json::Value = serde_json::from_str(&events)?;
    let result = serde_json::from_str::<Vec<HueEvent>>(&events);

    match result {
        Ok(events) => {
            let update_data_vec: Vec<UpdateData> = events
                .into_iter()
                .flat_map(|event| {
                    let HueEvent::Update(event) = event;

                    event.data
                })
                .collect();

            // We only want light_updates to contain the the result of applying
            // all incoming device state updates. This way we don't spam mqtt
            // with the Hue bridge's "halfway" state updates.
            //
            // Hue's eventsource API splits a light update into multiple
            // "UpdateData" chunks, with each chunk containing the change to a
            // single field. So if we send one HTTP request simultaneously
            // changing a light's power state, color and brightness, you will
            // get back three events, one for each field.
            let mut light_updates: HashMap<String, MqttDevice> = HashMap::new();
            for data in update_data_vec
                .iter()
                .filter(|data| matches!(data, UpdateData::Light(_)))
            {
                let mut mqtt_devices = mqtt_devices.write().await;
                let mqtt_device = data.to_mqtt_device(&mqtt_devices);

                if let Some(mqtt_device) = &mqtt_device {
                    // Store device state as computed from previous state and
                    // the event being handled
                    mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device.clone());
                    light_updates.insert(mqtt_device.id.clone(), mqtt_device.clone());
                }
            }

            // We want sensor_updates to contain all intermediate device states.
            // The reason is that we want to inform mqtt clients of situations
            // where sensor state rapidly changes between two values (such as
            // when pressing a button in rapid succession).
            //
            // For example, if a switch is pressed four times with just under
            // 1s between presses, due to the Hue bridge's debouncing of the
            // eventsource API events, we get sent the following sequence of
            // messages of the switch pressed state:
            //
            // [true], [false, true, false], [true, false], [true, false]
            //
            // If we were to only forward the trailing value of each message,
            // mqtt would only see [true, false, false, false].

            let sensor_updates: Vec<MqttDevice> = {
                let mut mqtt_devices = mqtt_devices.write().await;
                update_data_vec
                    .iter()
                    .filter(|data| {
                        (matches!(data, UpdateData::Button(_)) && !ignore_buttons)
                            | matches!(data, UpdateData::Motion(_))
                            | matches!(data, UpdateData::Temperature(_))
                            | matches!(data, UpdateData::LightLevel(_))
                    })
                    .filter(|data| match data {
                        UpdateData::Button(button) => {
                            // Ignore all other button presses from the
                            // eventsource API. Button resource polling will
                            // handle the other cases.
                            button.button.last_event == "initial_press"
                        }
                        _ => true,
                    })
                    // Only filter data if ignore_buttons flag is set to false and event is button related, otherwise we ignore any button updates for now
                    // Sensors are filtered as normal
                    .filter_map(|data| {
                        let mqtt_device = data.to_mqtt_device(&mqtt_devices);

                        if let Some(mqtt_device) = &mqtt_device {
                            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device.clone());
                        }

                        mqtt_device
                    })
                    .collect()
            };

            let updates = light_updates
                .into_values()
                .chain(sensor_updates.into_iter())
                .collect();

            Ok(updates)
        }
        Err(e) => {
            eprintln!(
                "Got unknown event:\n{}",
                serde_json::to_string_pretty(&serde_json_value)?
            );
            Err(eyre!(e))
        }
    }
}
