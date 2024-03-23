use std::{collections::HashMap, sync::Arc, time::Duration};

use eyre::{OptionExt, Result};
use futures::StreamExt;
use tokio::{
    sync::{Notify, RwLock},
    time::{timeout, Instant},
};

use crate::{
    mqtt::mqtt_device::{publish_mqtt_devices, MqttDevice},
    protocols::{
        eventsource::{mk_eventsource_stream, PinnedEventSourceStream},
        https::HyperHttpsClient,
        mqtt::MqttClient,
    },
    settings::Settings,
};

use super::{
    event_data::handle_incoming_hue_events, init_state::init_state_to_mqtt_devices,
    polling::poll_hue_buttons, rest::HueState,
};

async fn read_and_handle_eventsource_event(
    settings: &Settings,
    mqtt_client: &MqttClient,
    prev_event_t: &Arc<RwLock<Option<Instant>>>,
    mqtt_devices: &Arc<RwLock<HashMap<String, MqttDevice>>>,
    notify: &Arc<Notify>,
    eventsource_stream: &mut PinnedEventSourceStream,
) -> Result<()> {
    let e = eventsource_stream
        .next()
        .await
        .ok_or_eyre("Error while opening eventsource stream")??;

    // Ignore non-event messages
    let eventsource_client::SSE::Event(e) = e else {
        return Ok(());
    };

    // Check whether we should be ignoring button events
    let ignore_buttons = {
        let prev_event_t = prev_event_t.read().await;
        prev_event_t
            .map(|prev_event_t| prev_event_t.elapsed() < Duration::from_millis(1500))
            .unwrap_or(false)
    };

    let result = handle_incoming_hue_events(mqtt_devices, e.data, ignore_buttons).await;

    {
        let mut prev_event_t = prev_event_t.write().await;
        *prev_event_t = Some(Instant::now());
    }

    // Ignore errors in the eventsource event handling
    let mqtt_devices = match result {
        Ok(mqtt_devices) => mqtt_devices,
        Err(e) => {
            eprintln!("Error handling incoming Hue eventsource event: {e:?}");
            return Ok(());
        }
    };

    // Send a notification to the polling task that an event has just arrived
    notify.notify_one();

    let result = publish_mqtt_devices(mqtt_client, settings, mqtt_devices).await;

    if let Err(e) = result {
        eprintln!("Error publishing mqtt devices: {e:?}");
    }

    Ok(())
}

pub async fn eventsource_loop(
    settings: &Settings,
    mqtt_client: &MqttClient,
    https_client: &HyperHttpsClient,
    prev_event_t: &Arc<RwLock<Option<Instant>>>,
    mqtt_devices: &Arc<RwLock<HashMap<String, MqttDevice>>>,
    notify: &Arc<Notify>,
) -> Result<()> {
    let mut eventsource_stream = mk_eventsource_stream(settings, https_client)?;

    loop {
        let future = read_and_handle_eventsource_event(
            settings,
            mqtt_client,
            prev_event_t,
            mqtt_devices,
            notify,
            &mut eventsource_stream,
        );

        timeout(
            Duration::from_secs(settings.hue_bridge.eventsource_timeout_seconds),
            future,
        )
        .await??
    }
}

pub fn start_hue_events_loop(
    settings: &Settings,
    mqtt_client: &MqttClient,
    https_client: &HyperHttpsClient,
    init_state: &HueState,
) {
    let mqtt_client = mqtt_client.clone();
    let settings = settings.clone();
    let https_client = https_client.clone();
    let init_state = init_state.clone();

    // Somewhat annoyingly, the Hue eventsource endpoint returns changed fields
    // of a device in individual chunks. We need to persist these changes across
    // incoming events to be able to piece together current device state.
    let mqtt_devices = Arc::new(RwLock::new(init_state_to_mqtt_devices(&init_state)));

    // Notify channel is used to send a notification to the polling task that a
    // Hue bridge event of any kind was received
    let notify = Arc::new(Notify::new());
    let prev_event_t: Arc<RwLock<Option<Instant>>> = Default::default();

    {
        let https_client = https_client.clone();
        let mqtt_client = mqtt_client.clone();
        let mqtt_devices = mqtt_devices.clone();
        let settings = settings.clone();
        let notify = notify.clone();
        let prev_event_t = prev_event_t.clone();

        tokio::spawn(async move {
            loop {
                let result = eventsource_loop(
                    &settings,
                    &mqtt_client,
                    &https_client,
                    &prev_event_t,
                    &mqtt_devices,
                    &notify,
                )
                .await;

                if let Err(e) = result {
                    eprintln!(
                        "Error encountered in eventsource loop: {:?}, reconnecting",
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }

    tokio::spawn(async move {
        loop {
            // Wait for incoming event notifications
            notify.notified().await;

            // Sleep some time between the event arriving and starting to poll - it
            // is unlikely that state has changed this quickly
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            let mut interval = tokio::time::interval(Duration::from_millis(250));
            let prev_event_t = *prev_event_t.read().await;

            if let Some(prev_event_t) = prev_event_t {
                // Start polling for Hue bridge button state
                while prev_event_t.elapsed() < Duration::from_millis(1500) {
                    interval.tick().await;

                    let result =
                        poll_hue_buttons(&settings, &mqtt_client, &https_client, &mqtt_devices)
                            .await;

                    if let Err(e) = result {
                        eprintln!("{:?}", e);
                    }
                }
            }
        }
    });
}
