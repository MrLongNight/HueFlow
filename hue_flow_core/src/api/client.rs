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

        let url = format!("https://{}/api", ip);
        let resp = client.post(&url).json(&body).send().await?;

        // The Hue API returns a JSON array: [{"success": {...}}] or [{"error": {...}}]
        let items: Vec<RegisterResponseItem> = resp.json().await?;

        if let Some(item) = items.first() {
            match item {
                RegisterResponseItem::Success { success } => {
                    Ok(HueConfig {
                        bridge_ip: ip.to_string(),
                        username: success.username.clone(),
                        client_key: success.clientkey.clone(),
                        entertainment_group_id: "".to_string(), // Initial empty value
                    })
                }
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
