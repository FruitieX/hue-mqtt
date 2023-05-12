use std::{collections::HashMap, sync::Arc};

use super::{
    init_state::publish_hue_state,
    rest::{button::get_hue_buttons, get_hue_state},
};
use crate::{
    mqtt::mqtt_device::{publish_mqtt_device, MqttDevice},
    protocols::{https::HyperHttpsClient, mqtt::MqttClient},
    settings::Settings,
};
use color_eyre::Result;
use tokio::sync::RwLock;

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
pub fn start_hue_state_poll_loop(
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

/// This function polls the Hue bridge's Button resource API and publishes
/// detected button state changes to MQTT.
pub async fn poll_hue_buttons(
    settings: &Settings,
    mqtt_client: &MqttClient,
    https_client: &HyperHttpsClient,
    mqtt_devices: &Arc<RwLock<HashMap<String, MqttDevice>>>,
) -> Result<()> {
    let poll_result = get_hue_buttons(settings, https_client).await?;

    // Collect changed mqtt_devices
    let changed_mqtt_devices: Vec<MqttDevice> = {
        let mut mqtt_devices = mqtt_devices.write().await;
        let mut result = vec![];

        for button in poll_result {
            let mqtt_device = mqtt_devices.get_mut(&button.id);

            // Ignore already seen button reports
            if let (Some(updated), Some(button)) = (
                mqtt_device.as_ref().and_then(|x| x.updated.as_ref()),
                &button.button,
            ) {
                if updated == &button.button_report.updated {
                    continue;
                }
            }

            // Ignore junk data
            if let Some(button) = &button.button {
                if button.button_report.updated == "1970-01-01T00:00:00.000Z" {
                    continue;
                }
            }

            // Check if button state matches previously seen sensor value
            if let (Some(mqtt_device), Some(button)) = (mqtt_device, &button.button) {
                match (
                    mqtt_device.sensor_value.as_deref(),
                    button.is_pressed(),
                    mqtt_device.updated.as_ref(),
                ) {
                    (Some("false"), true, _) => {
                        mqtt_device.sensor_value = Some(true.to_string());
                        result.push(mqtt_device.clone());
                    }
                    (Some("true"), false, _) => {
                        mqtt_device.sensor_value = Some(false.to_string());
                        result.push(mqtt_device.clone());
                    }
                    (Some("false"), false, Some(updated)) => {
                        // We seem to have missed a false -> true -> false transition, let's fake a sensor_value of "true"
                        if updated != &button.button_report.updated {
                            mqtt_device.sensor_value = Some(true.to_string());
                            result.push(mqtt_device.clone());
                            mqtt_device.sensor_value = Some(false.to_string());
                            result.push(mqtt_device.clone());
                        }
                    }
                    (Some("true"), true, Some(updated)) => {
                        // We seem to have missed a true -> false -> true transition, let's fake a sensor_value of "false"
                        if updated != &button.button_report.updated {
                            mqtt_device.sensor_value = Some(false.to_string());
                            result.push(mqtt_device.clone());
                            mqtt_device.sensor_value = Some(true.to_string());
                            result.push(mqtt_device.clone());
                        }
                    }

                    _ => {}
                };

                mqtt_device.updated = Some(button.button_report.updated.clone());
            }
        }

        result
    };

    // Publish changed mqtt_devices to the broker
    for mqtt_device in changed_mqtt_devices {
        let publish_result = publish_mqtt_device(mqtt_client, settings, &mqtt_device).await;

        if let Err(e) = publish_result {
            eprintln!("{:?}", e);
        }
    }

    Ok(())
}
