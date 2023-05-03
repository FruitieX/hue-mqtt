use color_eyre::Result;
use rand::{distributions::Alphanumeric, Rng};
use rumqttc::{AsyncClient, MqttOptions, QoS};
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

    client
        .subscribe(
            settings.mqtt.light_topic_set.replace("{id}", "+"),
            QoS::AtMostOnce,
        )
        .await?;

    let mqtt_client = MqttClient {
        client,
        unhandled_messages,
        notify,
    };

    {
        let mqtt_client = mqtt_client.clone();

        task::spawn(async move {
            loop {
                while let Ok(event) = eventloop.poll().await {
                    let res = handle_incoming_mqtt_event(event, &mqtt_client).await;

                    if let Err(e) = res {
                        eprintln!("Error while handling MQTT event: {:?}", e);
                    }
                }
            }
        });
    }

    Ok(mqtt_client)
}
