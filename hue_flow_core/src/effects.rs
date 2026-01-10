use crate::audio_interface::AudioSpectrum;
use crate::models::{LightNode, LightState};

pub trait LightEffect: Send + Sync {
    fn apply(&mut self, spectrum: &AudioSpectrum, lights: &[LightNode]) -> Vec<LightState>;
}

pub struct PulseEffect;

impl LightEffect for PulseEffect {
    fn apply(&mut self, spectrum: &AudioSpectrum, lights: &[LightNode]) -> Vec<LightState> {
        let brightness = (spectrum.energy * 255.0).min(255.0) as u8;

        lights.iter().map(|node| {
            // Simple pulse: all lights same color based on energy
            // Red on bass, Green on mids, Blue on highs

            let r = (spectrum.bass * 255.0).min(255.0) as u8;
            let g = (spectrum.mids * 255.0).min(255.0) as u8;
            let b = (spectrum.highs * 255.0).min(255.0) as u8;

            // Mix with overall energy
            let r = r.saturating_add(brightness / 3);
            let g = g.saturating_add(brightness / 3);
            let b = b.saturating_add(brightness / 3);

            // Parse ID (assuming numeric ID for Hue Entertainment)
            let id = node.id.parse::<u8>().unwrap_or(0);

            LightState {
                id,
                r,
                g,
                b,
            }
        }).collect()
    }
}

pub struct MultiBandEffect;

impl LightEffect for MultiBandEffect {
    fn apply(&mut self, spectrum: &AudioSpectrum, lights: &[LightNode]) -> Vec<LightState> {
         lights.iter().map(|node| {
            let id = node.id.parse::<u8>().unwrap_or(0);

            // Map spatially based on X position (-1.0 to 1.0)
            // Left (x < -0.3): Bass (Red)
            // Right (x > 0.3): Highs (Blue)
            // Center: Mids (Green)

            let (r, g, b) = if node.x < -0.3 {
                 // Left - Bass
                 ((spectrum.bass * 255.0).min(255.0) as u8, 0, 0)
            } else if node.x > 0.3 {
                 // Right - Highs
                 (0, 0, (spectrum.highs * 255.0).min(255.0) as u8)
            } else {
                 // Center - Mids
                 (0, (spectrum.mids * 255.0).min(255.0) as u8, 0)
            };

            LightState {
                id,
                r,
                g,
                b,
            }
        }).collect()
    }
}
