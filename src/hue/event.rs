use std::collections::HashMap;

use color_eyre::Result;
use eyre::eyre;
use futures::TryStreamExt;
use palette::{FromColor, Hsv, Yxy};
use serde::Deserialize;

use crate::{
    mqtt_device::MqttDevice,
    protocols::{
        eventsource::PinnedEventSourceStream,
        mqtt::{publish_mqtt_device, MqttClient},
    },
    settings::Settings,
};

use super::{
    init_state::init_state_to_mqtt_devices,
    rest::{
        light::{ColorData, ColorTemperatureData, DimmingData, OnData},
        HueState,
    },
};

#[derive(Deserialize, Debug, Clone)]
struct ButtonData {
    last_event: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ButtonUpdateData {
    id: String,
    button: ButtonData,
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
struct DevicePowerData {}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum UpdateData {
    Button(ButtonUpdateData),
    Light(LightUpdateData),
    Motion(MotionUpdateData),

    // Ignored updates
    Temperature,        // Temperature sensor update
    LightLevel,         // Light sensor update
    DevicePower,        // Battery level update
    GroupedLight,       // Light groups update
    ZigbeeConnectivity, // Connectivity issue update
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

fn hue_event_data_to_mqtt_device(
    data: UpdateData,
    mqtt_devices: &mut HashMap<String, MqttDevice>,
) -> Option<MqttDevice> {
    match data {
        UpdateData::Button(button) => {
            let sensor_value = match button.button.last_event.as_str() {
                "short_release" | "long_release" => Some(false),
                "initial_press" => Some(true),
                _ => None,
            };

            if let Some(sensor_value) = sensor_value {
                let mut mqtt_device = mqtt_devices.get_mut(&button.id)?;

                mqtt_device.sensor_value = Some(sensor_value.to_string());

                return Some(mqtt_device.clone());
            }
        }
        UpdateData::Motion(motion) => {
            let mut mqtt_device = mqtt_devices.get_mut(&motion.id)?;

            mqtt_device.sensor_value = Some(motion.motion.motion.to_string());

            return Some(mqtt_device.clone());
        }
        UpdateData::Light(light) => {
            let mut mqtt_device = mqtt_devices.get_mut(&light.id)?;

            if let Some(color) = light.color {
                let mut hsv = Hsv::from_color(Yxy::new(color.xy.x, color.xy.y, 1.0));
                hsv.value = 1.0;

                mqtt_device.color = Some(hsv);
            }

            if let Some(ColorTemperatureData { mirek: Some(mirek) }) = light.color_temperature {
                let cct = 1_000_000.0 / mirek;
                mqtt_device.cct = Some(cct);
            }

            if let Some(on) = light.on {
                mqtt_device.power = Some(on.on);
            }

            if let Some(dimming) = light.dimming {
                mqtt_device.brightness = Some(dimming.brightness / 100.0)
            }

            return Some(mqtt_device.clone());
        }

        _ => {}
    };

    None
}

pub fn try_parse_hue_events(
    mqtt_devices: &mut HashMap<String, MqttDevice>,
    events: String,
) -> Result<Vec<MqttDevice>> {
    let serde_json_value: serde_json::Value = serde_json::from_str(&events)?;
    let result = serde_json::from_str::<Vec<HueEvent>>(&events);

    // Collect devices with updates to be sent to MQTT
    let mut updated_mqtt_devices = HashMap::new();

    match result {
        Ok(events) => {
            for event in events {
                let HueEvent::Update(event) = event;

                for data in event.data {
                    let mqtt_device = hue_event_data_to_mqtt_device(data, mqtt_devices);

                    if let Some(mqtt_device) = mqtt_device {
                        updated_mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device.clone());
                    }
                }
            }

            Ok(updated_mqtt_devices.values().cloned().collect())
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

pub fn start_eventsource_events_loop(
    mut eventsource_stream: PinnedEventSourceStream,
    settings: &Settings,
    mqtt_client: &MqttClient,
    init_state: &HueState,
) {
    let mqtt_client = mqtt_client.clone();
    let settings = settings.clone();
    let init_state = init_state.clone();

    // Somewhat annoyingly, the Hue eventsource endpoint returns all changed
    // fields of a device in individual chunks. We need to persist these changes
    // across incoming events.
    let mut mqtt_devices = init_state_to_mqtt_devices(&init_state);

    tokio::spawn(async move {
        while let Ok(Some(e)) = eventsource_stream.try_next().await {
            if let eventsource_client::SSE::Event(e) = e {
                let result = try_parse_hue_events(&mut mqtt_devices, e.data);
                match result {
                    Ok(mqtt_devices) => {
                        for mqtt_device in mqtt_devices {
                            let result =
                                publish_mqtt_device(&mqtt_client, &settings, &mqtt_device).await;

                            if let Err(e) = result {
                                eprintln!("{:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{:?}", e);
                    }
                }
            }
        }
    });
}
