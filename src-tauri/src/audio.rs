use std::sync::{Arc, Mutex};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, FromSample, Sample, SampleFormat, Stream, StreamConfig,
};
use serde::Serialize;

use crate::error::{AuralFlowError, Result};

const WHISPER_SAMPLE_RATE: u32 = 16_000;
const MAX_RECORDING_SECONDS: usize = 600;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct RecordedAudio {
    pub samples: Vec<f32>,
}

struct ActiveRecording {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    source_rate: u32,
    channels: u16,
}

#[derive(Default)]
pub struct AudioRecorder {
    active: Option<ActiveRecording>,
}

impl AudioRecorder {
    pub fn start(&mut self) -> Result<AudioInfo> {
        if self.active.is_some() {
            return Err(AuralFlowError::Audio("ya hay una grabación activa".into()));
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AuralFlowError::Audio("no se encontró un micrófono".into()))?;
        let device_name = device.to_string();
        let supported = device
            .default_input_config()
            .map_err(|error| AuralFlowError::Audio(format!("configuración no disponible: {error}")))?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let samples = Arc::new(Mutex::new(Vec::new()));
        let max_samples =
            config.sample_rate as usize * config.channels as usize * MAX_RECORDING_SECONDS;
        let stream = match sample_format {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &config, Arc::clone(&samples), max_samples)?
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &config, Arc::clone(&samples), max_samples)?
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &config, Arc::clone(&samples), max_samples)?
            }
            format => {
                return Err(AuralFlowError::Audio(format!(
                    "formato de micrófono no compatible: {format:?}"
                )))
            }
        };
        stream
            .play()
            .map_err(|error| AuralFlowError::Audio(format!("no se pudo iniciar: {error}")))?;

        let info = AudioInfo {
            device_name,
            sample_rate: config.sample_rate,
            channels: config.channels,
        };
        self.active = Some(ActiveRecording {
            stream,
            samples,
            source_rate: config.sample_rate,
            channels: config.channels,
        });
        Ok(info)
    }

    pub fn stop(&mut self) -> Result<RecordedAudio> {
        let active = self
            .active
            .take()
            .ok_or_else(|| AuralFlowError::Audio("no hay una grabación activa".into()))?;
        drop(active.stream);
        let interleaved = active
            .samples
            .lock()
            .map_err(|_| AuralFlowError::Audio("buffer de audio bloqueado".into()))?
            .clone();
        if interleaved.is_empty() {
            return Err(AuralFlowError::Audio("el micrófono no entregó audio".into()));
        }

        let mono = downmix_to_mono(&interleaved, active.channels as usize);
        let samples = resample_linear(&mono, active.source_rate, WHISPER_SAMPLE_RATE);
        Ok(RecordedAudio { samples })
    }
}

fn build_stream<T>(
    device: &Device,
    config: &StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    max_samples: usize,
) -> Result<Stream>
where
    T: Sample + cpal::SizedSample,
    f32: FromSample<T>,
{
    let error_callback = |error| eprintln!("AuralFlow audio stream error: {error}");
    device
        .build_input_stream(
            *config,
            move |input: &[T], _| {
                if let Ok(mut destination) = samples.lock() {
                    let remaining = max_samples.saturating_sub(destination.len());
                    destination.extend(
                        input
                            .iter()
                            .copied()
                            .take(remaining)
                            .map(f32::from_sample),
                    );
                }
            },
            error_callback,
            None,
        )
        .map_err(|error| AuralFlowError::Audio(format!("no se pudo abrir el micrófono: {error}")))
}

fn downmix_to_mono(input: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return input.to_vec();
    }
    input
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn resample_linear(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if input.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return input.to_vec();
    }

    let output_length = ((input.len() as u64 * target_rate as u64) / source_rate as u64) as usize;
    let ratio = source_rate as f64 / target_rate as f64;
    let mut output = Vec::with_capacity(output_length);
    for index in 0..output_length {
        let source_position = index as f64 * ratio;
        let left = source_position.floor() as usize;
        let right = (left + 1).min(input.len() - 1);
        let fraction = (source_position - left as f64) as f32;
        output.push(input[left] * (1.0 - fraction) + input[right] * fraction);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_stereo_frames() {
        assert_eq!(downmix_to_mono(&[1.0, 3.0, -1.0, 1.0], 2), vec![2.0, 0.0]);
    }

    #[test]
    fn resamples_to_expected_length() {
        let input = vec![0.0; 48_000];
        assert_eq!(resample_linear(&input, 48_000, 16_000).len(), 16_000);
    }

    #[test]
    fn leaves_native_rate_unchanged() {
        let input = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_linear(&input, 16_000, 16_000), input);
    }
}
