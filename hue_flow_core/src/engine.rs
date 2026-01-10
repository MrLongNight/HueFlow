use tokio::sync::mpsc;
use crate::audio_interface::{AudioProcessor, AudioSpectrum};
use crate::models::{LightNode, LightState};
use crate::effects::LightEffect;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit, scaling::divide_by_N, windows::hann_window};

pub struct EntertainmentEngine {
    audio_rx: mpsc::Receiver<Vec<f32>>,
    light_tx: mpsc::Sender<Vec<LightState>>,
    lights: Vec<LightNode>,
    effect: Box<dyn LightEffect>,
    sample_rate: u32,
}

struct SimpleAudioProcessor {
    sample_rate: u32,
}

impl SimpleAudioProcessor {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
        }
    }
}

impl AudioProcessor for SimpleAudioProcessor {
    fn process(&mut self, samples: &[f32]) -> AudioSpectrum {
        // Use spectrum-analyzer crate
        // samples length should be power of 2, e.g. 1024 or 2048.
        // The cli buffers 1024 samples.

        // hann window
        let window = hann_window(samples);

        // FFT
        let spectrum_result = samples_fft_to_spectrum(
            &window,
            self.sample_rate,
            FrequencyLimit::All,
            Some(&divide_by_N),
        );

        match spectrum_result {
            Ok(spec) => {
                // Map to Bass, Mids, Highs
                // Bass: 20-250 Hz
                // Mids: 250-4000 Hz
                // Highs: 4000-20000 Hz

                let mut bass_sum = 0.0;
                let mut bass_count = 0;
                let mut mids_sum = 0.0;
                let mut mids_count = 0;
                let mut highs_sum = 0.0;
                let mut highs_count = 0;

                for (freq, val) in spec.data() {
                    let freq_val = freq.val();
                    let val_f32 = val.val();

                    if freq_val >= 20.0 && freq_val < 250.0 {
                        bass_sum += val_f32;
                        bass_count += 1;
                    } else if freq_val >= 250.0 && freq_val < 4000.0 {
                        mids_sum += val_f32;
                        mids_count += 1;
                    } else if freq_val >= 4000.0 && freq_val < 20000.0 {
                        highs_sum += val_f32;
                        highs_count += 1;
                    }
                }

                // Simple averaging and scaling (very rough)
                // Need AGC ideally, but for now just multiply by a constant factor to make it visible
                let gain = 100.0;

                let bass = if bass_count > 0 { (bass_sum / bass_count as f32) * gain } else { 0.0 };
                let mids = if mids_count > 0 { (mids_sum / mids_count as f32) * gain } else { 0.0 };
                let highs = if highs_count > 0 { (highs_sum / highs_count as f32) * gain } else { 0.0 };

                let energy = (bass + mids + highs) / 3.0;

                AudioSpectrum {
                    bass: bass.min(1.0),
                    mids: mids.min(1.0),
                    highs: highs.min(1.0),
                    energy: energy.min(1.0),
                }
            }
            Err(_) => AudioSpectrum::default(),
        }
    }
}

impl EntertainmentEngine {
    pub fn new(
        audio_rx: mpsc::Receiver<Vec<f32>>,
        light_tx: mpsc::Sender<Vec<LightState>>,
        lights: Vec<LightNode>,
        effect: Box<dyn LightEffect>,
        sample_rate: u32,
    ) -> Self {
        Self {
            audio_rx,
            light_tx,
            lights,
            effect,
            sample_rate,
        }
    }

    pub async fn run(mut self) {
        let mut processor = SimpleAudioProcessor::new(self.sample_rate);

        while let Some(samples) = self.audio_rx.recv().await {
            // Process audio
            let spectrum = processor.process(&samples);

            // Apply effect
            let light_states = self.effect.apply(&spectrum, &self.lights);

            // Send to streamer
            if let Err(_) = self.light_tx.send(light_states).await {
                break; // Receiver dropped
            }
        }
    }
}
