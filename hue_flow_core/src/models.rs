use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HueConfig {
    pub ip: String,
    pub username: String,
    pub client_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    // Optional: active state, brightness, etc. can be added later
}
