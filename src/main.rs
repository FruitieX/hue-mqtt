use color_eyre::Result;
use hue::event::start_eventsource_events_loop;
use hue::init_state::start_hue_state_loop;
use hue::rest::get_hue_state;
use protocols::eventsource::mk_eventsource_stream;
use protocols::https::mk_hyper_https_client;
use protocols::mqtt::{mk_mqtt_client, start_mqtt_events_loop};

use crate::settings::read_settings;

mod hue;
mod mqtt_device;
mod protocols;
mod settings;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let settings = read_settings()?;
    let mqtt_client = mk_mqtt_client(&settings).await?;
    let https_client = mk_hyper_https_client(&settings)?;
    let eventsource_stream = mk_eventsource_stream(&settings, &https_client)?;

    let init_state = get_hue_state(&settings, &https_client).await?;

    start_hue_state_loop(&settings, &https_client, &mqtt_client);
    start_mqtt_events_loop(&mqtt_client, &settings, &https_client);
    start_eventsource_events_loop(eventsource_stream, &settings, &mqtt_client, &https_client, &init_state);

    tokio::signal::ctrl_c().await?;

    Ok(())
}
