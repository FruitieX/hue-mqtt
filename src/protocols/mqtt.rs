#![allow(clippy::redundant_closure_call)]

use color_eyre::Result;
use rand::{distributions::Alphanumeric, Rng};
use rumqttc::{AsyncClient, MqttOptions};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::{
    sync::{Notify, RwLock},
    task,
};

use crate::{
    mqtt::{events::handle_incoming_mqtt_event, mqtt_device::MqttDevice},
    settings::Settings,
};

type UnhandledMessages = Arc<RwLock<VecDeque<MqttDevice>>>;

#[derive(Clone)]
pub struct MqttClient {
    pub client: AsyncClient,
    pub unhandled_messages: UnhandledMessages,
    pub notify: Arc<Notify>,
}

pub async fn mk_mqtt_client(settings: &Settings) -> Result<MqttClient> {
    let random_string: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();

    let mut options = MqttOptions::new(
        format!("{}-{}", settings.mqtt.id, random_string),
        settings.mqtt.host.clone(),
        settings.mqtt.port,
    );
    options.set_keep_alive(Duration::from_secs(5));
    let (client, mut eventloop) = AsyncClient::new(options, 10);

    let unhandled_messages: UnhandledMessages = Default::default();
    let notify = Arc::new(Notify::new());

    let mqtt_client = MqttClient {
        client,
        unhandled_messages,
        notify,
    };

    {
        let mqtt_client = mqtt_client.clone();
        let settings = settings.clone();

        task::spawn(async move {
            loop {
                let notification = eventloop.poll().await;

                let res = (|| async {
                    handle_incoming_mqtt_event(notification?, &mqtt_client, &settings).await?;

                    Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
                })()
                .await;

                if let Err(e) = res {
                    eprintln!("MQTT error: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        });
    }

    Ok(mqtt_client)
}
