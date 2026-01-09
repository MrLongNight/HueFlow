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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hue_config_serialization() {
        let config = HueConfig {
            ip: "192.168.1.100".to_string(),
            username: "user".to_string(),
            client_key: "key".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let decoded: HueConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.ip, "192.168.1.100");
        assert_eq!(decoded.username, "user");
        assert_eq!(decoded.client_key, "key");
    }

    #[test]
    fn test_light_node_serialization() {
        let node = LightNode {
            id: "1".to_string(),
            x: 0.5,
            y: 0.1,
            z: -0.5,
        };

        let json = serde_json::to_string(&node).unwrap();
        let decoded: LightNode = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.id, "1");
        assert_eq!(decoded.x, 0.5);
    }
}
