use std::collections::HashMap;

use super::rest::{get_hue_state, light::ColorTemperatureData, HueState};
use crate::{
    mqtt_device::{MqttDevice, MqttDeviceBuilder},
    protocols::{https::HyperHttpsClient, mqtt::MqttClient},
    settings::Settings,
};
use color_eyre::Result;
use palette::{FromColor, Hsv, Yxy};
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
                let mut hsv = Hsv::from_color(Yxy::new(color.xy.x, color.xy.y, 1.0));
                hsv.value = 1.0;
                builder.color(hsv);
            }

            if let Some(ColorTemperatureData { mirek: Some(mirek) }) = light.color_temperature {
                let cct = 1_000_000.0 / mirek;
                builder.cct(cct);
            }

            let mqtt_device = builder.build().unwrap();
            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device);
        }
    }

    for button in init_state.buttons.values() {
        let device = init_state.devices.get(&button.owner.rid);

        if let Some(device) = device {
            let mqtt_device = MqttDeviceBuilder::default()
                .id(button.id.clone())
                .name(format!(
                    "{} button {}",
                    device.metadata.name, button.metadata.control_id
                ))
                .sensor_value("false")
                .build()
                .unwrap();

            mqtt_devices.insert(mqtt_device.id.clone(), mqtt_device);
        }
    }

    for motion in init_state.motion.values() {
        let device = init_state.devices.get(&motion.owner.rid);

        if let Some(device) = device {
            let mqtt_device = MqttDeviceBuilder::default()
                .id(motion.id.clone())
                .name(device.metadata.name.clone())
                .sensor_value("false")
                .build()
                .unwrap();

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

/// Periodically poll for hue state and publish to MQTT.
///
/// The zigbee network may drop state change messages, and we will never know
/// about that happening through only the eventsource API.
/// 
/// TODO:
/// Another perhaps better idea for solving this problem would be:
///
/// - Every time we request a light state change for a specific device, keep
/// listening for events from the eventsource api
/// - If an "acknowledgement" of the state change arrives from the device, we
/// know that the bulb has been set to the correct state and we don't need to do
/// anything else
/// - If no event is received within say 2 seconds, re-send the state change
/// request
pub fn start_hue_state_loop(
    settings: &Settings,
    https_client: &HyperHttpsClient,
    mqtt_client: &MqttClient,
) {
    let settings = settings.clone();
    let https_client = https_client.clone();
    let mqtt_client = mqtt_client.clone();

    tokio::spawn(async move {
        loop {
            let state = get_hue_state(&settings, &https_client).await;

            let result = match state {
                Ok(state) => publish_hue_state(&settings, &mqtt_client, &state).await,
                Err(e) => Err(e),
            };

            if let Err(e) = result {
                eprintln!("{:?}", e);
            };

            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
}
