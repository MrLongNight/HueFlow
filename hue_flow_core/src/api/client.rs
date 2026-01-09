use crate::api::error::HueError;
use crate::models::HueConfig;
use serde::{Deserialize, Serialize};

pub struct HueClient;

#[derive(Serialize)]
struct RegisterBody<'a> {
    devicetype: &'a str,
    generateclientkey: bool,
}

#[derive(Deserialize)]
struct RegisterSuccess {
    username: String,
    clientkey: String,
}

#[derive(Deserialize)]
struct HueErrorResponse {
    #[serde(rename = "type")]
    error_type: i32,
    description: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RegisterResponseItem {
    Success { success: RegisterSuccess },
    Error { error: HueErrorResponse },
}

impl HueClient {
    pub async fn register_user(ip: &str, devicename: &str) -> Result<HueConfig, HueError> {
        // Use danger_accept_invalid_certs because Hue Bridge uses self-signed certs
        // In a production environment, we might want to pin the certificate or use a CA if available.
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;

        let body = RegisterBody {
            devicetype: devicename,
            generateclientkey: true,
        };

        // If 'ip' already contains "http", use it as base url (useful for mocks), otherwise assume it's just an IP
        let url = if ip.starts_with("http") {
            format!("{}/api", ip)
        } else {
            format!("http://{}/api", ip)
        };

        let resp = client.post(&url).json(&body).send().await?;

        // The Hue API returns a JSON array: [{"success": {...}}] or [{"error": {...}}]
        let items: Vec<RegisterResponseItem> = resp.json().await?;

        if let Some(item) = items.first() {
            match item {
                RegisterResponseItem::Success { success } => Ok(HueConfig {
                    ip: ip.to_string(),
                    username: success.username.clone(),
                    client_key: success.clientkey.clone(),
                }),
                RegisterResponseItem::Error { error } => {
                    if error.error_type == 101 {
                        Err(HueError::LinkButtonNotPressed)
                    } else {
                        Err(HueError::ApiError(error.description.clone()))
                    }
                }
            }
        } else {
            Err(HueError::ApiError(
                "Empty response from Hue Bridge".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_register_user_success() {
        let mock_server = MockServer::start().await;

        let response_body = json!([{
            "success": {
                "username": "new_developer",
                "clientkey": "someclientkey"
            }
        }]);

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let result = HueClient::register_user(&mock_server.uri(), "test_device").await;

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.username, "new_developer");
        assert_eq!(config.client_key, "someclientkey");
    }

    #[tokio::test]
    async fn test_register_user_link_button_not_pressed() {
        let mock_server = MockServer::start().await;

        let response_body = json!([{
            "error": {
                "type": 101,
                "address": "",
                "description": "link button not pressed"
            }
        }]);

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let result = HueClient::register_user(&mock_server.uri(), "test_device").await;

        match result {
            Err(HueError::LinkButtonNotPressed) => (), // passed
            _ => panic!("Expected LinkButtonNotPressed error"),
        }
    }

    #[tokio::test]
    async fn test_register_user_other_error() {
        let mock_server = MockServer::start().await;

        let response_body = json!([{
            "error": {
                "type": 7,
                "address": "/username",
                "description": "invalid value, , for parameter, username"
            }
        }]);

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let result = HueClient::register_user(&mock_server.uri(), "test_device").await;

        match result {
            Err(HueError::ApiError(desc)) => {
                assert_eq!(desc, "invalid value, , for parameter, username")
            }
            _ => panic!("Expected ApiError"),
        }
    }

    #[tokio::test]
    async fn test_parse_register_success() {
        let json = json!([{
            "success": {
                "username": "myuser",
                "clientkey": "mykey"
            }
        }]);

        let items: Vec<RegisterResponseItem> = serde_json::from_value(json).unwrap();
        if let RegisterResponseItem::Success { success } = &items[0] {
            assert_eq!(success.username, "myuser");
            assert_eq!(success.clientkey, "mykey");
        } else {
            panic!("Expected success");
        }
    }

    #[tokio::test]
    async fn test_parse_register_error_101() {
        let json = json!([{
            "error": {
                "type": 101,
                "address": "",
                "description": "link button not pressed"
            }
        }]);

        let items: Vec<RegisterResponseItem> = serde_json::from_value(json).unwrap();
        if let RegisterResponseItem::Error { error } = &items[0] {
            assert_eq!(error.error_type, 101);
        } else {
            panic!("Expected error");
        }
    }
}
