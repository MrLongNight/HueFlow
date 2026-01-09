use reqwest::Client;
use serde::Deserialize;
use crate::api::error::HueError;

#[derive(Deserialize)]
struct NUPnPDevice {
    #[serde(rename = "internalipaddress")]
    internal_ip_address: String,
    #[allow(dead_code)]
    id: String,
}

pub async fn discover_bridge() -> Result<String, HueError> {
    let client = Client::new();
    let resp = client.get("https://discovery.meethue.com")
        .send()
        .await?;

    let devices: Vec<NUPnPDevice> = resp.json().await?;

    if let Some(device) = devices.first() {
        Ok(device.internal_ip_address.clone())
    } else {
        Err(HueError::DiscoveryFailed)
    }
}
