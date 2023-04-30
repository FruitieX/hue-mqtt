# hue-mqtt

This program synchronizes a Philips Hue bridge with an MQTT broker.

## Setup

- Follow the [Philips Hue API V2 getting started guide](https://developers.meethue.com/develop/hue-api-v2/getting-started/)
  (requires free user account) until you have obtained your bridge's IP address, and an app key ("username").
- Copy `Settings.example.toml` to `Settings.toml`.
- Edit `Settings.toml` with values matching your setup.
- Try running hue-mqtt with `cargo run`. If your bridge runs recent enough firmware, the program should now launch without errors.
- If you get SSL / certificate verification errors, run `openssl s_client -showcerts -connect <IP address of Hue bridge>`
  and add the displayed self signed certificate into your Settings toml under `[hue_bridge]` and rerun the program. Example:

  ```
  [hue_bridge]

  self_signed_cert = """
  -----BEGIN CERTIFICATE-----
  MIICOTCCAd+gAwIBAgIHF4j//kEd4DAKBggqhkjOPQQDAjA+MQswCQYDVQQGEwJO
  TDEUMBIGA1UECgwLUGhpbGlwcyBIdWUxGTAXBgNVBAMMEDAwMTc4OGZmZmU0MTFk
  ZTAwIhgPMjAxNzAxMDEwMDAwMDBaGA8yMDM4MDEwMTAwMDAwMFowPjELMAkGA1UE
  BhMCTkwxFDASBgNVBAoMC1BoaWxpcHMgSHVlMRkwFwYDVQQDDBAwMDE3ODhmZmZl
  NDExZGUwMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAErMLMdLgT1y7RAU78CkTX
  Db2SY8c5Ltnkz1QAH4DeBJ8bqVB6duudFPUxO4qOW+rN1XC/putZAAfHcRFc3Op/
  VKOBwzCBwDAMBgNVHRMBAf8EAjAAMB0GA1UdDgQWBBSklzU+PppzO7VFJwbcerGi
  NmnJXDBsBgNVHSMEZTBjgBSklzU+PppzO7VFJwbcerGiNmnJXKFCpEAwPjELMAkG
  A1UEBhMCTkwxFDASBgNVBAoMC1BoaWxpcHMgSHVlMRkwFwYDVQQDDBAwMDE3ODhm
  ZmZlNDExZGUwggcXiP/+QR3gMA4GA1UdDwEB/wQEAwIFoDATBgNVHSUEDDAKBggr
  BgEFBQcDATAKBggqhkjOPQQDAgNIADBFAiEAhcKx6Aq0qlEJep1jgDT1Aw7zkUsG
  VqEF6VFI3VscmpkCIEqI+YyngggI/eInoGEFWHLW2ljIRZsmAOXa6JtWyavI
  -----END CERTIFICATE-----
  """
  ```

Now you should be able to view all your Hue devices through e.g. [MQTT Explorer](http://mqtt-explorer.com/) once connected to the same MQTT broker.

### Setting Up Mosquitto 

- Ensure Docker Desktop is installed and is running

- Pull the official Mosquitto Docker image using the following command:

```
$ docker pull eclipse-mosquitto
```

- Run a container using the new image:

```
$ docker run -it -p 1883:1883 -p 9001:9001 -v mosquitto.conf:/mosquitto/config/mosquitto.conf -v /mosquitto/data -v /mosquitto/log eclipse-mosquitto
```

### Setting Up MQTT Explorer

- Install [MQTT Explorer](http://mqtt-explorer.com/)

- Connect to the MQTT broker by configuring the connection

  - Setting the protocol to mqtt://
  - Setting the host to localhost
  - Setting the port to 1883

## Topics

The default MQTT topics are as follows:

- `/home/{lights,sensors}/hue/{id}`: Current state of the device serialized as JSON
- `/home/lights/hue/{id}/set`: Sets state of the light to given JSON

## State messages

MQTT messages follow this structure, serialized as JSON:

```
struct MqttDevice {
    pub id: String,
    pub name: String,
    pub power: Option<bool>,
    pub brightness: Option<f32>,
    pub cct: Option<f32>,
    pub color: Option<Hsv>,
    pub transition_ms: Option<f32>,
    pub sensor_value: Option<String>,
}
```

Example light state:

```
{
  "id": "d68b5135-0e71-4333-ad3d-07b635144422",
  "name": "Office",
  "power": null,
  "brightness": 0.5,
  "cct": null,
  "color": {
    "hue": 31.238605,
    "saturation": 0.7411992,
    "value": 1
  },
  "transition_ms": null,
  "sensor_value": null
}
```

Example sensor state (2nd dimmer switch button pressed in):

```
{
  "id": "a8f6b7e3-80a1-45ee-9af6-ef9b6204c72d",
  "name": "Office switch button 2",
  "power": null,
  "brightness": null,
  "cct": null,
  "color": null,
  "transition_ms": null,
  "sensor_value": "true"
}
```
