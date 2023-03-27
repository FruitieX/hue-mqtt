use color_eyre::Result;
use es::Client;
use eventsource_client as es;
use eyre::eyre;
use futures::Stream;
use std::{pin::Pin, time::Duration};

use crate::{protocols::https::HyperHttpsClient, settings::Settings};

pub type EventSourceStream = dyn Stream<Item = Result<eventsource_client::SSE, eventsource_client::Error>>
    + std::marker::Send
    + std::marker::Sync;

pub type PinnedEventSourceStream = Pin<Box<EventSourceStream>>;

pub fn mk_eventsource_stream(
    settings: &Settings,
    client: &HyperHttpsClient,
) -> Result<PinnedEventSourceStream> {
    let eventsource_client = es::ClientBuilder::for_url(&format!(
        "https://{}/eventstream/clip/v2",
        settings.hue_bridge.addr
    ))
    .map_err(|e| {
        eyre!(
            "Failed to build HTTP client for given URL. Check your hue bridge addr config. {:?}",
            e
        )
    })?
    .header("hue-application-key", &settings.hue_bridge.appkey)
    .map_err(|e| {
        eyre!(
            "Failed to set hue-application-key header. Check your hue bridge appkey config. {:?}",
            e
        )
    })?
    .header("Accept", "text/event-stream")
    .unwrap()
    .reconnect(
        es::ReconnectOptions::reconnect(true)
            .retry_initial(false)
            .delay(Duration::from_secs(2))
            .backoff_factor(2)
            .delay_max(Duration::from_secs(60))
            .build(),
    )
    .build_with_http_client(client.clone());

    Ok(eventsource_client.stream())
}
