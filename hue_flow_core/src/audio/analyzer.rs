use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit, scaling};
use spectrum_analyzer::windows::hann_window;
use tracing::error;

#[derive(Debug, Clone, Default)]
pub struct AudioSpectrum {
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub energy: f32,
}

pub struct FftAnalyzer {
    fft_size: usize,
    sampling_rate: u32,
    max_val: f32,
}

impl FftAnalyzer {
    /// Creates a new FftAnalyzer.
    /// Note: sampling_rate defaults to 44100Hz. Use `set_sampling_rate` if different.
    pub fn new(fft_size: usize) -> Self {
        Self {
            fft_size,
            sampling_rate: 44100,
            max_val: 0.01,
        }
    }

    pub fn set_sampling_rate(&mut self, rate: u32) {
        self.sampling_rate = rate;
    }

    pub fn process(&mut self, samples: &[f32]) -> AudioSpectrum {
        // Energy (RMS)
        let sum_sq: f32 = samples.iter().map(|&x| x * x).sum();
        let energy = if !samples.is_empty() {
            (sum_sq / samples.len() as f32).sqrt()
        } else {
            0.0
        };

        // Prepare samples for FFT
        // spectrum_analyzer usually expects power of 2 length.
        // We will pad or truncate to fft_size.
        let mut input = samples.to_vec();
        if input.len() < self.fft_size {
            input.resize(self.fft_size, 0.0);
        } else if input.len() > self.fft_size {
            input.truncate(self.fft_size);
        }

        let windowed = hann_window(&input);

        // Perform FFT
        let spectrum_res = samples_fft_to_spectrum(
            &windowed,
            self.sampling_rate,
            FrequencyLimit::Range(20.0, 20000.0),
            Some(&scaling::divide_by_N),
        );

        let spectrum = match spectrum_res {
            Ok(s) => s,
            Err(e) => {
                // If FFT fails (e.g. empty samples or wrong size), log and return energy only
                error!("FFT processing failed: {:?}", e);
                return AudioSpectrum {
                    bass: 0.0,
                    mids: 0.0,
                    highs: 0.0,
                    energy,
                };
            }
        };

        // Aggregate frequencies
        let mut bass_sum = 0.0;
        let mut mids_sum = 0.0;
        let mut highs_sum = 0.0;

        // data() returns a collection of (Frequency, FrequencyValue)
        for (freq, val) in spectrum.data().iter() {
            let f = freq.val();
            let v = val.val();

            if f >= 20.0 && f < 250.0 {
                bass_sum += v;
            } else if f >= 250.0 && f < 4000.0 {
                mids_sum += v;
            } else if f >= 4000.0 && f <= 20000.0 {
                highs_sum += v;
            }
        }

        // AGC / Rolling Max Normalization
        let current_peak = bass_sum.max(mids_sum).max(highs_sum);

        if current_peak > self.max_val {
            self.max_val = current_peak;
        } else {
            // Decay max_val slowly to adapt to quieter sections
            self.max_val *= 0.99;
            // Prevent division by zero or extremely small values
            if self.max_val < 0.001 {
                self.max_val = 0.001;
            }
        }

        AudioSpectrum {
            bass: (bass_sum / self.max_val).clamp(0.0, 1.0),
            mids: (mids_sum / self.max_val).clamp(0.0, 1.0),
            highs: (highs_sum / self.max_val).clamp(0.0, 1.0),
            energy,
        }
    }
}
