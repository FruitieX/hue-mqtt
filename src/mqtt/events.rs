use color_eyre::Result;
use eyre::eyre;
use rumqttc::QoS;

use crate::{
    hue::rest::light::{put_hue_light, PutResponse},
    mqtt::mqtt_device::MqttDevice,
    protocols::{https::HyperHttpsClient, mqtt::MqttClient},
    settings::Settings,
};

pub async fn handle_incoming_mqtt_event(
    event: rumqttc::Event,
    mqtt_client: &MqttClient,
    settings: &Settings,
) -> Result<()> {
    match event {
        rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_)) => {
            mqtt_client
                .client
                .subscribe(
                    settings.mqtt.light_topic_set.replace("{id}", "+"),
                    QoS::AtMostOnce,
                )
                .await?;
        }
        rumqttc::Event::Incoming(rumqttc::Packet::Publish(msg)) => {
            let device: MqttDevice = serde_json::from_slice(&msg.payload)?;

            // Push device update to the unhandled messages
            // queue, removing any existing unhandled messages
            // for the same device.
            let mut unhandled_messages = mqtt_client.unhandled_messages.write().await;
            unhandled_messages.retain(|d: &MqttDevice| d.id != device.id);
            unhandled_messages.push_back(device);

            // Notify Hue bridge communication task that there are new messages
            mqtt_client.notify.notify_one();
        }
        _ => {}
    }

    Ok(())
}

pub fn start_mqtt_events_loop(
    mqtt_client: &MqttClient,
    settings: &Settings,
    https_client: &HyperHttpsClient,
) {
    let unhandled_messages = mqtt_client.unhandled_messages.clone();
    let notify = mqtt_client.notify.clone();

    let settings = settings.clone();
    let https_client = https_client.clone();

    tokio::spawn(async move {
        loop {
            let next_message = {
                let mut unhandled_messages = unhandled_messages.write().await;
                unhandled_messages.pop_front()
            };

            match next_message {
                Some(message) => {
                    let result =
                        process_next_mqtt_message(message, settings.clone(), https_client.clone())
                            .await;

                    if let Err(e) = result {
                        eprintln!("Error while processing MQTT message: {:?}", e);
                    }
                }
                None => {
                    // Wait until we get notified that there are new messages.
                    notify.notified().await;
                }
            }
        }
    });
}

async fn process_next_mqtt_message(
    mqtt_device: MqttDevice,
    settings: Settings,
    https_client: HyperHttpsClient,
) -> Result<Option<PutResponse>> {
    let result = put_hue_light(&settings, &https_client, &mqtt_device).await?;

    if !result.errors.is_empty() {
        Err(eyre!(
            "Error while sending PUT to Hue light resource (name: {}):\n{:#?}",
            mqtt_device.name,
            result.errors
        ))
    } else {
        Ok(Some(result))
    }
}
