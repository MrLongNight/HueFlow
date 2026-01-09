use std::collections::HashMap;
use crate::audio_interface::AudioSpectrum;
use crate::models::LightNode;
use std::cmp::Ordering;

pub trait LightEffect: Send + Sync {
    fn update(&mut self, audio: &AudioSpectrum, nodes: &[LightNode]) -> HashMap<u8, (u8, u8, u8)>;
}

pub struct PulseEffect {
    pub color: (u8, u8, u8),
}

impl PulseEffect {
    pub fn new(color: (u8, u8, u8)) -> Self {
        Self { color }
    }
}

impl LightEffect for PulseEffect {
    fn update(&mut self, audio: &AudioSpectrum, nodes: &[LightNode]) -> HashMap<u8, (u8, u8, u8)> {
        let brightness = (audio.bass * audio.energy).clamp(0.0, 1.0);
        let r = (self.color.0 as f32 * brightness) as u8;
        let g = (self.color.1 as f32 * brightness) as u8;
        let b = (self.color.2 as f32 * brightness) as u8;

        let mut result = HashMap::new();
        for node in nodes {
            if let Ok(id) = node.id.parse::<u8>() {
                result.insert(id, (r, g, b));
            }
        }
        result
    }
}

pub struct MultiBandEffect;

impl MultiBandEffect {
    pub fn new() -> Self {
        Self
    }
}

impl LightEffect for MultiBandEffect {
    fn update(&mut self, audio: &AudioSpectrum, nodes: &[LightNode]) -> HashMap<u8, (u8, u8, u8)> {
        let mut result = HashMap::new();
        if nodes.is_empty() {
            return result;
        }

        // Check if we have position data (at least one node has non-zero coordinate)
        let has_positions = nodes.iter().any(|n| n.x.abs() > 0.001 || n.y.abs() > 0.001 || n.z.abs() > 0.001);

        if !has_positions {
             // Modulo ID fallback
             for node in nodes {
                 if let Ok(id) = node.id.parse::<u8>() {
                     let (val, color) = match id % 3 {
                         0 => (audio.bass, (255, 0, 0)), // Bass -> Red
                         1 => (audio.mids, (0, 255, 0)), // Mids -> Green
                         2 => (audio.highs, (0, 0, 255)), // Highs -> Blue
                         _ => (0.0, (0, 0, 0)),
                     };
                     let brightness = val.clamp(0.0, 1.0);
                     let r = (color.0 as f32 * brightness) as u8;
                     let g = (color.1 as f32 * brightness) as u8;
                     let b = (color.2 as f32 * brightness) as u8;
                     result.insert(id, (r, g, b));
                 }
             }
        } else {
            // Sort by X
            let mut sorted_nodes: Vec<&LightNode> = nodes.iter().collect();
            sorted_nodes.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal));

            let count = sorted_nodes.len();

            for (i, node) in sorted_nodes.iter().enumerate() {
                if let Ok(id) = node.id.parse::<u8>() {
                    let section = if count < 3 {
                        i // if 1 node: 0 -> Bass. if 2 nodes: 0->Bass, 1->Mids.
                    } else {
                        // i ranges from 0 to count-1
                        // partition into 3
                        (i * 3) / count
                    };

                    let (val, color) = match section {
                        0 => (audio.bass, (255, 0, 0)),
                        1 => (audio.mids, (0, 255, 0)),
                        _ => (audio.highs, (0, 0, 255)),
                    };

                    let brightness = val.clamp(0.0, 1.0);
                    let r = (color.0 as f32 * brightness) as u8;
                    let g = (color.1 as f32 * brightness) as u8;
                    let b = (color.2 as f32 * brightness) as u8;
                    result.insert(id, (r, g, b));
                }
            }
        }
        result
    }
}
