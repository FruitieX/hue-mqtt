use std::collections::HashMap;

use self::{
    button::{get_hue_buttons, ButtonData},
    device::{get_hue_devices, DeviceData},
    light::{get_hue_lights, LightData},
    motion::{get_hue_motion, MotionData},
};
use crate::{protocols::https::HyperHttpsClient, settings::Settings};
use color_eyre::Result;

pub mod button;
pub mod common;
pub mod device;
pub mod light;
pub mod motion;

#[derive(Clone, Debug)]
pub struct HueState {
    pub devices: HashMap<String, DeviceData>,
    pub buttons: HashMap<String, ButtonData>,
    pub lights: HashMap<String, LightData>,
    pub motion: HashMap<String, MotionData>,
}

pub async fn get_hue_state(settings: &Settings, client: &HyperHttpsClient) -> Result<HueState> {
    let devices = get_hue_devices(settings, client).await?;
    let buttons = get_hue_buttons(settings, client).await?;
    let lights = get_hue_lights(settings, client).await?;
    let motion = get_hue_motion(settings, client).await?;

    // Fix some data quality issues
    let buttons: Vec<ButtonData> = buttons
        .into_iter()
        .map(|mut x| {
            if let Some(button) = &mut x.button {
                if button.button_report.updated == "1970-01-01T00:00:00.000Z" {
                    button.last_event = "short_release".to_string();
                    button.button_report.event = "short_release".to_string();
                }
            }

            x
        })
        .collect();

    // Put each device in a HashMap where the key is the device id, as this
    // makes it faster to find a device by ID.
    let devices = devices.into_iter().map(|x| (x.id.clone(), x)).collect();
    let buttons = buttons.into_iter().map(|x| (x.id.clone(), x)).collect();
    let lights = lights.into_iter().map(|x| (x.id.clone(), x)).collect();
    let motion = motion.into_iter().map(|x| (x.id.clone(), x)).collect();

    Ok(HueState {
        devices,
        buttons,
        lights,
        motion,
    })
}
