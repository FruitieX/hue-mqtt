use color_eyre::Result;
use eyre::eyre;
use hyper::{Request, Uri};
use serde::{Deserialize, Serialize};
use tokio_rustls::rustls::{
    client::danger::ServerCertVerified,
    pki_types::{CertificateDer, ServerName, UnixTime},
    Error, RootCertStore, ClientConfig,
};

use crate::settings::Settings;

// see https://quinn-rs.github.io/quinn/quinn/certificate.html
#[derive(Debug)]
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
}

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::prelude::v1::Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        Error,
    > {
        todo!()
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::prelude::v1::Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        Error,
    > {
        todo!()
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        todo!()
    }
}

pub fn mk_hyper_https_client(settings: &Settings) -> Result<HyperHttpsClient> {
    // https://github.com/spietika/restson-rust/pull/20
    let mut http = hyper::client::conn::http1::Builder::new();

    // This is the Signify CA certificate for Hue bridges, from:
    // https://developers.meethue.com/develop/application-design-guidance/using-https/
    const HUE_CA_CERT: &[u8] = include_bytes!("hue_ca_cert.pem");

    // Allow overriding the trusted CA certificate for older bridge firmware that still use self signed certs
    let cert_bytes = match &settings.hue_bridge.self_signed_cert {
        Some(cert) => cert.as_bytes().to_vec(),
        None => HUE_CA_CERT.to_vec(),
    };

    let cert_parsed = rustls_pemfile::certs(&mut cert_bytes.as_slice())
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("Failed to parse certificate"))??;

    let mut root_store = RootCertStore::empty();
    root_store.add(cert_parsed)?;

    let mut client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    // Allow disabling host name verification
    // See https://docs.rs/native-tls/0.2.2/native_tls/struct.TlsConnectorBuilder.html#method.danger_accept_invalid_hostnames
    if let Some(true) = settings.hue_bridge.disable_host_name_verification {
        client_config
            .dangerous()
            .set_certificate_verifier(SkipServerVerification::new());
    }

    let https =
        hyper_rustls::HttpsConnectorBuilder::new().with_tls_config(client_config).https_or_http().enable_http1().build();
        //<hyper::client::conn::http1::Builder>::from((http, client_config));

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
