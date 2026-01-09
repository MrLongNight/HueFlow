use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HueConfig {
    pub bridge_ip: String,
    pub username: String,
    pub client_key: String,
    pub entertainment_group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightNode {
    pub id: u8,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeInfo {
    pub id: String,
    pub ip: String,
}
