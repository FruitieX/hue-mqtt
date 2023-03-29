use std::collections::HashMap;

use color_eyre::Result;
use eyre::eyre;
use futures::TryStreamExt;
use palette::{FromColor, Hsv, Yxy};
use serde::Deserialize;

use crate::{
    mqtt_device::{MqttDevice, MqttDeviceBuilder},
    protocols::{
        eventsource::PinnedEventSourceStream,
        mqtt::{publish_mqtt_device, MqttClient},
    },
    settings::Settings,
};

use super::{
    init_state::init_state_to_mqtt_devices,
    rest::{
        common::Owner,
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
    owner: Owner,
}

#[derive(Deserialize, Debug, Clone)]
struct LightUpdateData {
    id: String,
    on: Option<OnData>,
    dimming: Option<DimmingData>,
    color: Option<ColorData>,
    color_temperature: Option<ColorTemperatureData>,
    owner: Owner,
}

#[derive(Deserialize, Debug, Clone)]
struct MotionData {
    motion: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct MotionUpdateData {
    id: String,
    motion: MotionData,
    owner: Owner,
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

pub fn try_parse_hue_events(
    init_state: &HueState,
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
                    match data {
                        UpdateData::Button(button) => {
                            let device = init_state.devices.get(&button.owner.rid);
                            let button_device = init_state.buttons.get(&button.id);

                            if let (Some(device), Some(button_device)) = (device, button_device) {
                                let sensor_value = match button.button.last_event.as_str() {
                                    "short_release" | "long_release" => Some(false),
                                    "initial_press" => Some(true),
                                    _ => None,
                                };

                                if let Some(sensor_value) = sensor_value {
                                    let mqtt_device = mqtt_devices
                                        .entry(button.id.clone())
                                        .or_insert_with(|| {
                                            // TODO: this should be unneeded
                                            MqttDeviceBuilder::default()
                                                .id(button.id.clone())
                                                .name(format!(
                                                    "{} button {}",
                                                    device.metadata.name,
                                                    button_device.metadata.control_id
                                                ))
                                                .build()
                                                .unwrap()
                                        });

                                    mqtt_device.sensor_value = Some(sensor_value.to_string());

                                    updated_mqtt_devices
                                        .insert(mqtt_device.id.clone(), mqtt_device.clone());
                                }
                            }
                        }
                        UpdateData::Motion(motion) => {
                            let device = init_state.devices.get(&motion.owner.rid);

                            if let Some(device) = device {
                                let mqtt_device =
                                    mqtt_devices.entry(motion.id.clone()).or_insert_with(|| {
                                        // TODO: this should be unneeded
                                        MqttDeviceBuilder::default()
                                            .id(motion.id.clone())
                                            .name(device.metadata.name.clone())
                                            .build()
                                            .unwrap()
                                    });

                                mqtt_device.sensor_value = Some(motion.motion.motion.to_string());

                                updated_mqtt_devices
                                    .insert(mqtt_device.id.clone(), mqtt_device.clone());
                            }
                        }
                        UpdateData::Light(light) => {
                            let device = init_state.devices.get(&light.owner.rid);

                            if let Some(device) = device {
                                let mqtt_device =
                                    mqtt_devices.entry(light.id.clone()).or_insert_with(|| {
                                        // TODO: this should be unneeded
                                        MqttDeviceBuilder::default()
                                            .id(light.id.clone())
                                            .name(device.metadata.name.clone())
                                            .build()
                                            .unwrap()
                                    });

                                if let Some(color) = light.color {
                                    let mut hsv =
                                        Hsv::from_color(Yxy::new(color.xy.x, color.xy.y, 1.0));
                                    hsv.value = 1.0;

                                    mqtt_device.color = Some(hsv);
                                }

                                if let Some(ColorTemperatureData { mirek: Some(mirek) }) =
                                    light.color_temperature
                                {
                                    let cct = 1_000_000.0 / mirek;
                                    mqtt_device.cct = Some(cct);
                                }

                                if let Some(on) = light.on {
                                    mqtt_device.power = Some(on.on);
                                }

                                if let Some(dimming) = light.dimming {
                                    mqtt_device.brightness = Some(dimming.brightness / 100.0)
                                }

                                updated_mqtt_devices
                                    .insert(mqtt_device.id.clone(), mqtt_device.clone());
                            }
                        }

                        _ => {}
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
                let result = try_parse_hue_events(&init_state, &mut mqtt_devices, e.data);
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
