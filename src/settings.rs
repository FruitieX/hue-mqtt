use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct HueSettings {
    pub addr: String,
    pub appkey: String,
    pub self_signed_cert: Option<String>,
    pub disable_host_name_verification: Option<bool>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct MqttSettings {
    pub id: String,
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Settings {
    pub hue_bridge: HueSettings,
    pub mqtt: MqttSettings,
}

pub fn read_settings() -> Result<Settings, config::ConfigError> {
    config::Config::builder()
        .add_source(config::File::with_name("Settings"))
        .build()?
        .try_deserialize::<Settings>()
}
