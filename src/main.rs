use color_eyre::Result;
use hue::events::start_hue_events_loop;
use hue::polling::start_hue_state_poll_loop;
use hue::rest::get_hue_state;
use mqtt::events::start_mqtt_events_loop;
use protocols::https::mk_hyper_https_client;
use protocols::mqtt::mk_mqtt_client;

#[macro_use]
extern crate eyre;

#[macro_use]
extern crate log;

use crate::settings::read_settings;

mod hue;
mod mqtt;
mod protocols;
mod settings;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    pretty_env_logger::init();

    let settings = read_settings()?;
    let mqtt_client = mk_mqtt_client(&settings).await?;
    let https_client = mk_hyper_https_client(&settings)?;

    let init_state = get_hue_state(&settings, &https_client).await?;

    start_hue_state_poll_loop(&settings, &https_client, &mqtt_client);
    start_mqtt_events_loop(&mqtt_client, &settings, &https_client);
    start_hue_events_loop(&settings, &mqtt_client, &https_client, &init_state);

    tokio::signal::ctrl_c().await?;

    Ok(())
}
