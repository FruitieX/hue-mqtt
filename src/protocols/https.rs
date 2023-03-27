use color_eyre::Result;
use hyper::{Request, Uri};
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

pub type HyperHttpsClient = hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>;

pub fn mk_hyper_https_client(settings: &Settings) -> Result<HyperHttpsClient> {
    // https://github.com/spietika/restson-rust/pull/20
    let mut http = hyper::client::HttpConnector::new();
    http.enforce_http(false);

    let mut tls_connector_builder = native_tls::TlsConnector::builder();

    // This is the Signify CA certificate for Hue bridges, from:
    // https://developers.meethue.com/develop/application-design-guidance/using-https/
    const HUE_CA_CERT: &[u8] = include_bytes!("hue_ca_cert.pem");

    // Allow overriding the trusted CA certificate for older bridge firmware that still use self signed certs
    let cert_bytes = match &settings.hue_bridge.self_signed_cert {
        Some(cert) => cert.as_bytes().to_vec(),
        None => HUE_CA_CERT.to_vec(),
    };

    let certificate = native_tls::Certificate::from_pem(&cert_bytes)?;

    // Adds the certificate to the set of roots that the connector will trust.
    // See https://docs.rs/native-tls/0.2.2/native_tls/struct.TlsConnectorBuilder.html#method.add_root_certificate
    tls_connector_builder.add_root_certificate(certificate);

    // Allow disabling host name verification
    // See https://docs.rs/native-tls/0.2.2/native_tls/struct.TlsConnectorBuilder.html#method.danger_accept_invalid_hostnames
    if let Some(true) = settings.hue_bridge.disable_host_name_verification {
        tls_connector_builder.danger_accept_invalid_hostnames(true);
    }

    let tls_connector = tls_connector_builder.build()?;
    let https = hyper_tls::HttpsConnector::<hyper::client::HttpConnector>::from((
        http,
        tls_connector.into(),
    ));

    // Build the hyper client
    let client = hyper::Client::builder().build(https);

    Ok(client)
}

pub async fn mk_get_request<T: for<'a> Deserialize<'a>>(
    client: &HyperHttpsClient,
    settings: &Settings,
    uri: &Uri,
) -> Result<T> {
    let request = Request::builder()
        .method("GET")
        .header("hue-application-key", &settings.hue_bridge.appkey)
        .uri(uri)
        .body(hyper::Body::empty())?;

    let result = client.request(request).await?;
    let body_bytes = hyper::body::to_bytes(result.into_body()).await?;
    let de = &mut serde_json::Deserializer::from_slice(&body_bytes);
    let response: T = serde_path_to_error::deserialize(de)?;

    Ok(response)
}

pub async fn mk_put_request<RequestBody, ResponseBody>(
    client: &HyperHttpsClient,
    settings: &Settings,
    uri: &Uri,
    body: &RequestBody,
) -> Result<ResponseBody>
where
    RequestBody: Serialize,
    ResponseBody: for<'a> Deserialize<'a>,
{
    let body = serde_json::to_string(body)?;

    let request = Request::builder()
        .method("PUT")
        .header("hue-application-key", &settings.hue_bridge.appkey)
        .uri(uri)
        .body(body.into())?;

    let result = client.request(request).await?;
    let body_bytes = hyper::body::to_bytes(result.into_body()).await?;
    let de = &mut serde_json::Deserializer::from_slice(&body_bytes);
    let response: ResponseBody = serde_path_to_error::deserialize(de)?;

    Ok(response)
}
