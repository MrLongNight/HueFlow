use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use anyhow::{Context, Result};
use tokio::sync::mpsc;

pub struct AudioInput {
    _stream: cpal::Stream,
}

impl AudioInput {
    pub fn new(sender: mpsc::Sender<Vec<f32>>) -> Result<(Self, u32)> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .context("No input device available")?;

        let config = device.default_input_config()
            .context("Failed to get default input config")?;

        let sample_rate = config.sample_rate().0;

        tracing::info!("Using input device: {}", device.name()?);
        tracing::info!("Sample rate: {} Hz", sample_rate);

        let err_fn = |err| tracing::error!("an error occurred on stream: {}", err);

        let buffer_size = 1024;

        let mut sample_buffer: Vec<f32> = Vec::with_capacity(buffer_size);

        let tx = sender.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                 device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        sample_buffer.extend_from_slice(data);

                        while sample_buffer.len() >= buffer_size {
                            let chunk: Vec<f32> = sample_buffer.drain(0..buffer_size).collect();
                             match tx.blocking_send(chunk) {
                                 Ok(_) => {},
                                 Err(e) => eprintln!("Failed to send audio buffer: {}", e),
                             }
                        }
                    },
                    err_fn,
                    None
                )?
            },
            cpal::SampleFormat::I16 => {
                 device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &_| {
                        for &sample in data {
                            sample_buffer.push((sample as f32) / (i16::MAX as f32));
                        }
                         while sample_buffer.len() >= buffer_size {
                            let chunk: Vec<f32> = sample_buffer.drain(0..buffer_size).collect();
                             match tx.blocking_send(chunk) {
                                 Ok(_) => {},
                                 Err(e) => eprintln!("Failed to send audio buffer: {}", e),
                             }
                        }
                    },
                    err_fn,
                    None
                )?
            },
            cpal::SampleFormat::U16 => {
                 device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &_| {
                         for &sample in data {
                            sample_buffer.push(((sample as f32) - (u16::MAX as f32) / 2.0) / ((u16::MAX as f32) / 2.0));
                        }
                         while sample_buffer.len() >= buffer_size {
                            let chunk: Vec<f32> = sample_buffer.drain(0..buffer_size).collect();
                             match tx.blocking_send(chunk) {
                                 Ok(_) => {},
                                 Err(e) => eprintln!("Failed to send audio buffer: {}", e),
                             }
                        }
                    },
                    err_fn,
                    None
                )?
            },
             _ => return Err(anyhow::anyhow!("Unsupported sample format: {:?}.", config.sample_format())),
        };

        stream.play()?;

        Ok((AudioInput { _stream: stream }, sample_rate))
    }
}
