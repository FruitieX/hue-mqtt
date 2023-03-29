use color_eyre::Result;
use eyre::eyre;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{watch::Receiver, RwLock},
    task,
};

use crate::{
    hue::rest::light::{put_hue_light, PutResponse},
    mqtt_device::MqttDevice,
    settings::Settings,
};

use super::https::HyperHttpsClient;

pub type MqttRx = Arc<RwLock<Receiver<Option<MqttDevice>>>>;

#[derive(Clone)]
pub struct MqttClient {
    pub client: AsyncClient,
    pub rx: MqttRx,
}

pub async fn mk_mqtt_client(settings: &Settings) -> Result<MqttClient> {
    let mut options = MqttOptions::new(
        settings.mqtt.id.clone(),
        settings.mqtt.host.clone(),
        settings.mqtt.port,
    );
    options.set_keep_alive(Duration::from_secs(5));
    let (client, mut eventloop) = AsyncClient::new(options, 10);

    let (tx, rx) = tokio::sync::watch::channel(None);
    let tx = Arc::new(RwLock::new(tx));
    let rx = Arc::new(RwLock::new(rx));

    client
        .subscribe(
            settings.mqtt.light_topic_set.replace("{id}", "+"),
            QoS::AtMostOnce,
        )
        .await?;

    task::spawn(async move {
        loop {
            while let Ok(notification) = eventloop.poll().await {
                let mqtt_tx = tx.clone();

                let res = (|| async move {
                    if let rumqttc::Event::Incoming(rumqttc::Packet::Publish(msg)) = notification {
                        let device: MqttDevice = serde_json::from_slice(&msg.payload)?;

                        let tx = mqtt_tx.write().await;
                        tx.send(Some(device))?;
                    }

                    Ok::<(), Box<dyn std::error::Error>>(())
                })()
                .await;

                if let Err(e) = res {
                    eprintln!("MQTT error: {:?}", e);
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    Ok(MqttClient { client, rx })
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
    let mqtt_rx = mqtt_client.rx.clone();
    let settings = settings.clone();
    let https_client = https_client.clone();

    tokio::spawn(async move {
        loop {
            let result =
                process_next_mqtt_message(mqtt_rx.clone(), settings.clone(), https_client.clone())
                    .await;

            if let Err(e) = result {
                eprintln!("{:?}", e);
            }
        }
    });
}

async fn process_next_mqtt_message(
    mqtt_rx: MqttRx,
    settings: Settings,
    https_client: HyperHttpsClient,
) -> Result<PutResponse> {
    let mqtt_device = {
        let mut mqtt_rx = mqtt_rx.write().await;
        mqtt_rx
            .changed()
            .await
            .expect("Expected mqtt_rx channel to never close");
        let value = &*mqtt_rx.borrow();
        value
            .clone()
            .ok_or_else(|| eyre!("Expected to receive mqtt message from rx channel"))?
    };

    let result = put_hue_light(&settings, &https_client, &mqtt_device).await?;

    if !result.errors.is_empty() {
        Err(eyre!("{:#?}", result.errors))
    } else {
        Ok(result)
    }
}
