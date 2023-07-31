use std::collections::HashMap;

use super::rest::{light::ColorTemperatureData, HueState};
use crate::{
    mqtt::mqtt_device::{Capabilities, Ct, DeviceColor, MqttDevice, MqttDeviceBuilder, Xy},
    protocols::mqtt::MqttClient,
    settings::Settings,
};
use color_eyre::Result;
use rumqttc::QoS;

pub fn init_state_to_mqtt_devices(init_state: &HueState) -> HashMap<String, MqttDevice> {
    let mut mqtt_devices: HashMap<String, MqttDevice> = HashMap::new();

    for light in init_state.lights.values() {
        let device = init_state.devices.get(&light.owner.rid);

        if let Some(device) = device {
            let mut builder = MqttDeviceBuilder::default();
            builder
                .id(light.id.clone())
                .name(device.metadata.name.clone())
                .power(light.on.on);

            if let Some(dimming) = &light.dimming {
                builder.brightness(dimming.brightness / 100.0);
            }

            if let Some(color) = &light.color {
                builder.color(DeviceColor::Xy(Xy {
                    x: color.xy.x,
                    y: color.xy.y,
                }));
            }

            if let Some(ColorTemperatureData {
                mirek: Some(mirek), ..
            }) = light.color_temperature
            {
                let ct = (1_000_000.0 / mirek) as u16;
                builder.color(DeviceColor::Ct(Ct { ct }));
            }

            builder.capabilities(Capabilities {
                ct: light.color_temperature.as_ref().map(|ct| {
                    let min = ct
                        .mirek_schema
                        .as_ref()
                        .map(|s| s.mirek_minimum)
                        .unwrap_or(153.0);
                    let max = ct
                        .mirek_schema
                        .as_ref()
                        .map(|s| s.mirek_maximum)
                        .unwrap_or(500.0);

                    // min/max being flipped here is intentional, as mirek is
                    // inversely proportional to ct
                    let min_ct = (1_000_000.0 / max) as u16;
                    let max_ct = (1_000_000.0 / min) as u16;

                    min_ct..max_ct
                }),
                xy: light.color.is_some(),
            });

            let mqtt_device = builder.build().unwrap();
            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device);
        }
    }

    for button in init_state.buttons.values() {
        let device = init_state.devices.get(&button.owner.rid);

        if let Some(device) = device {
            let mut builder = MqttDeviceBuilder::default();

            builder.id(button.id.clone()).name(format!(
                "{} button {}",
                device.metadata.name, button.metadata.control_id
            ));

            if let Some(button_event) = &button.button {
                builder.sensor_value(button_event.is_pressed().to_string());
                builder.updated(button_event.button_report.updated.clone());
            }

            let mqtt_device = builder.build().unwrap();

            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device);
        }
    }

    for motion in init_state.motion.values() {
        let device = init_state.devices.get(&motion.owner.rid);

        if let Some(device) = device {
            let mut builder = MqttDeviceBuilder::default();

            builder
                .id(motion.id.clone())
                .name(device.metadata.name.clone());

            if let Some(motion_event) = &motion.motion {
                builder.sensor_value(motion_event.motion.to_string());
            }

            let mqtt_device = builder.build().unwrap();

            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device);
        }
    }

    for temperature in init_state.temperature.values() {
        let device = init_state.devices.get(&temperature.owner.rid);

        if let Some(device) = device {
            let mut builder = MqttDeviceBuilder::default();

            builder.id(temperature.id.clone()).name(format!(
                "{} {}",
                device.metadata.name.clone(),
                " temperature"
            ));

            if let Some(temperature_event) = &temperature.temperature {
                builder.sensor_value(temperature_event.temperature.to_string());
            }

            let mqtt_device = builder.build().unwrap();

            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device);
        }
    }

    for light_level in init_state.light_level.values() {
        let device = init_state.devices.get(&light_level.owner.rid);

        if let Some(device) = device {
            let mut builder = MqttDeviceBuilder::default();

            builder.id(light_level.id.clone()).name(format!(
                "{} {}",
                device.metadata.name.clone(),
                " light level"
            ));

            if let Some(light_level_event) = &light_level.light_level {
                builder.sensor_value(light_level_event.light_level.to_string());
            }

            let mqtt_device = builder.build().unwrap();

            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device);
        }
    }

    mqtt_devices
}

pub async fn publish_hue_state(
    settings: &Settings,
    mqtt_client: &MqttClient,
    hue_state: &HueState,
) -> Result<()> {
    let mqtt_devices = init_state_to_mqtt_devices(hue_state);

    // Publish initial state of each discovered device to MQTT
    for mqtt_device in mqtt_devices.values() {
        let topic_template = if mqtt_device.sensor_value.is_some() {
            &settings.mqtt.sensor_topic
        } else {
            &settings.mqtt.light_topic
        };

        let topic = topic_template.replace("{id}", &mqtt_device.id);

        let json = serde_json::to_string(&mqtt_device)?;

        mqtt_client
            .client
            .publish(topic, QoS::AtLeastOnce, true, json)
            .await?;
    }

    Ok(())
}
