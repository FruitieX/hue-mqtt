use color_eyre::Result;
use eyre::eyre;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::{
    sync::{Notify, RwLock},
    task,
};

use crate::{
    hue::rest::light::{put_hue_light, PutResponse},
    mqtt_device::MqttDevice,
    settings::Settings,
};

use super::https::HyperHttpsClient;

type UnhandledMessages = Arc<RwLock<VecDeque<MqttDevice>>>;

#[derive(Clone)]
pub struct MqttClient {
    pub client: AsyncClient,
    pub unhandled_messages: UnhandledMessages,
    pub notify: Arc<Notify>,
}

pub async fn mk_mqtt_client(settings: &Settings) -> Result<MqttClient> {
    let mut options = MqttOptions::new(
        settings.mqtt.id.clone(),
        settings.mqtt.host.clone(),
        settings.mqtt.port,
    );
    options.set_keep_alive(Duration::from_secs(5));
    let (client, mut eventloop) = AsyncClient::new(options, 10);

    let unhandled_messages: UnhandledMessages = Default::default();
    let notify = Arc::new(Notify::new());

    client
        .subscribe(
            settings.mqtt.light_topic_set.replace("{id}", "+"),
            QoS::AtMostOnce,
        )
        .await?;

    {
        let unhandled_messages = unhandled_messages.clone();
        let notify = notify.clone();

        task::spawn(async move {
            loop {
                while let Ok(notification) = eventloop.poll().await {
                    let unhandled_messages = unhandled_messages.clone();
                    let notify = notify.clone();

                    let res = (|| async move {
                        if let rumqttc::Event::Incoming(rumqttc::Packet::Publish(msg)) =
                            notification
                        {
                            let device: MqttDevice = serde_json::from_slice(&msg.payload)?;

                            // Push device update to the unhandled messages
                            // queue, removing any existing unhandled messages
                            // for the same device.
                            let mut unhandled_messages = unhandled_messages.write().await;
                            unhandled_messages.retain(|d: &MqttDevice| d.id != device.id);
                            unhandled_messages.push_back(device);

                            // Notify Hue bridge communication task that there are new messages
                            notify.notify_one();
                        }

                        Ok::<(), Box<dyn std::error::Error>>(())
                    })()
                    .await;

                    if let Err(e) = res {
                        eprintln!("MQTT error: {:?}", e);
                    }
                }
            }
        });
    }

    Ok(MqttClient {
        client,
        unhandled_messages,
        notify,
    })
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
