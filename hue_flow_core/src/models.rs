use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HueConfig {
    pub ip: String,
    pub username: String,
    pub client_key: String,
    pub entertainment_group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone)]
pub struct LightState {
    pub id: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
