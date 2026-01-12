use crate::api::error::HueError;
use crate::models::{HueConfig, LightNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GroupInfo {
    pub id: String,        // Numeric v1 API ID (for REST calls like set_stream_active)
    pub stream_id: String, // UUID for DTLS streaming (36 characters)
    pub name: String,
    pub lights: Vec<LightNode>,
}

#[derive(Deserialize)]
struct GroupListEntry {
    name: String,
    #[serde(rename = "type")]
    group_type: String,
}

#[derive(Deserialize)]
struct GroupDetails {
    locations: HashMap<String, [f64; 3]>, // LightID -> [x, y, z]
}

#[derive(Serialize)]
struct StreamStatus {
    active: bool,
}

#[derive(Serialize)]
struct StreamBody {
    stream: StreamStatus,
}

// V2 API structures for entertainment_configuration
#[derive(Deserialize, Debug)]
struct V2Response<T> {
    data: Vec<T>,
}

#[derive(Deserialize, Debug)]
struct V2EntertainmentConfig {
    id: String, // This is the UUID we need!
    metadata: V2Metadata,
}

#[derive(Deserialize, Debug)]
struct V2Metadata {
    name: String,
}

// Helper to build a client with insecure certs (Hue Bridge standard)
fn build_client() -> Result<reqwest::Client, HueError> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(HueError::Network)
}

pub async fn get_entertainment_groups(config: &HueConfig) -> Result<Vec<GroupInfo>, HueError> {
    let client = build_client()?;

    // Step 1: Get v1 groups (for locations and to enable streaming)
    let v1_url = format!(
        "https://{}/api/{}/groups",
        config.bridge_ip, config.username
    );

    let resp = client.get(&v1_url).send().await?;
    let groups_map: HashMap<String, GroupListEntry> = resp.json().await?;

    // Step 2: Get v2 entertainment_configuration (for UUIDs)
    let v2_url = format!(
        "https://{}/clip/v2/resource/entertainment_configuration",
        config.bridge_ip
    );

    let v2_resp = client
        .get(&v2_url)
        .header("hue-application-key", &config.username)
        .send()
        .await?;

    let v2_configs: V2Response<V2EntertainmentConfig> = v2_resp
        .json()
        .await
        .unwrap_or_else(|_| V2Response { data: vec![] });

    // Build a map of name -> UUID
    let mut name_to_uuid: HashMap<String, String> = HashMap::new();
    for cfg in &v2_configs.data {
        name_to_uuid.insert(cfg.metadata.name.clone(), cfg.id.clone());
    }

    let mut result = Vec::new();

    for (id, info) in groups_map {
        if info.group_type == "Entertainment" {
            // Fetch details for locations
            let details_url = format!(
                "https://{}/api/{}/groups/{}",
                config.bridge_ip, config.username, id
            );
            let details_resp = client.get(&details_url).send().await?;
            let details: GroupDetails = details_resp.json().await?;

            let mut lights = Vec::new();
            for (light_id, loc) in details.locations {
                lights.push(LightNode {
                    id: light_id,
                    x: loc[0],
                    y: loc[1],
                    z: loc[2],
                });
            }

            // Get the UUID from v2 API by matching the name
            let stream_id = name_to_uuid.get(&info.name).cloned().unwrap_or_else(|| {
                // Fallback: use the v1 ID (will likely not work, but better than panic)
                eprintln!(
                    "WARNING: Could not find v2 UUID for '{}', using v1 ID",
                    info.name
                );
                id.clone()
            });

            result.push(GroupInfo {
                id,
                stream_id,
                name: info.name,
                lights,
            });
        }
    }

    Ok(result)
}

pub async fn set_stream_active(
    config: &HueConfig,
    group_id: &str,
    active: bool,
) -> Result<(), HueError> {
    let client = build_client()?;
    let url = format!(
        "https://{}/api/{}/groups/{}",
        config.bridge_ip, config.username, group_id
    );

    let body = StreamBody {
        stream: StreamStatus { active },
    };

    let resp = client.put(&url).json(&body).send().await?;

    let response_text = resp.text().await?;

    // Check if response contains error
    if response_text.contains("\"error\"") {
        return Err(HueError::ApiError(format!(
            "Failed to activate stream: {}",
            response_text
        )));
    }

    Ok(())
}

pub async fn flash_light(config: &HueConfig, light_id: &str) -> Result<(), HueError> {
    let client = build_client()?;
    let url = format!(
        "https://{}/api/{}/lights/{}/state",
        config.bridge_ip, config.username, light_id
    );

    // Flash the light once (select effect)
    let body = serde_json::json!({
        "alert": "select"
    });

    let resp = client.put(&url).json(&body).send().await?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(HueError::ApiError(format!(
            "Failed to flash light: {}",
            resp.status()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_parse_group_details() {
        let json = json!({
            "name": "Entertainment Area 1",
            "type": "Entertainment",
            "locations": {
                "1": [0.5, 0.0, 1.0],
                "2": [-0.5, 0.0, 1.0]
            }
        });

        let details: GroupDetails = serde_json::from_value(json).unwrap();
        assert_eq!(details.locations.len(), 2);
        assert_eq!(details.locations["1"], [0.5, 0.0, 1.0]);
    }
}
