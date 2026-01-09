use crate::api::error::HueError;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct NUPnPDevice {
    #[serde(rename = "internalipaddress")]
    internal_ip_address: String,
    #[allow(dead_code)]
    id: String,
}

pub async fn discover_bridge() -> Result<String, HueError> {
    discover_bridge_internal("https://discovery.meethue.com").await
}

/// Internal function to allow testing with a mock server
pub async fn discover_bridge_internal(url: &str) -> Result<String, HueError> {
    let client = Client::new();
    let resp = client.get(url).send().await?;

    let devices: Vec<NUPnPDevice> = resp.json().await?;

    if let Some(device) = devices.first() {
        Ok(device.internal_ip_address.clone())
    } else {
        Err(HueError::DiscoveryFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_discover_bridge_success() {
        let mock_server = MockServer::start().await;

        let response_body = json!([{
            "id": "001788FFFE100491",
            "internalipaddress": "192.168.2.23"
        }]);

        Mock::given(method("GET"))
            .and(path("/")) // The root of the mock server
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let result = discover_bridge_internal(&mock_server.uri()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "192.168.2.23");
    }

    #[tokio::test]
    async fn test_discover_bridge_failed_empty() {
        let mock_server = MockServer::start().await;

        let response_body = json!([]);

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let result = discover_bridge_internal(&mock_server.uri()).await;

        match result {
            Err(HueError::DiscoveryFailed) => (),
            _ => panic!("Expected DiscoveryFailed error"),
        }
    }
}
