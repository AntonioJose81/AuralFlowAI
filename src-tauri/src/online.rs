use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use crate::error::{AuralFlowError, Result};

const GROQ_TRANSCRIPTIONS_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

#[derive(Deserialize)]
struct GroqTranscript {
    text: String,
}

pub async fn transcribe_groq(samples: &[f32], language: &str, api_key: &str) -> Result<String> {
    if api_key.trim().is_empty() {
        return Err(AuralFlowError::Online(
            "falta la clave API de Groq; añádela en Preferencias".into(),
        ));
    }

    let wav = pcm16_wav(samples, 16_000);
    let file = Part::bytes(wav)
        .file_name("auralflow.wav")
        .mime_str("audio/wav")
        .map_err(|error| AuralFlowError::Online(error.to_string()))?;
    let mut form = Form::new()
        .part("file", file)
        .text("model", "whisper-large-v3-turbo")
        .text("response_format", "json")
        .text("temperature", "0");
    if language != "auto" {
        form = form.text("language", language.to_owned());
    }

    let response = reqwest::Client::new()
        .post(GROQ_TRANSCRIPTIONS_URL)
        .bearer_auth(api_key.trim())
        .multipart(form)
        .send()
        .await
        .map_err(|error| AuralFlowError::Online(format!("no se pudo conectar con Groq: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        let detail = detail.chars().take(240).collect::<String>();
        return Err(AuralFlowError::Online(format!(
            "Groq respondió {status}: {detail}"
        )));
    }

    let transcript: GroqTranscript = response
        .json()
        .await
        .map_err(|error| AuralFlowError::Online(format!("respuesta inválida de Groq: {error}")))?;
    Ok(transcript.text.trim().to_owned())
}

fn pcm16_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let pcm = ((*sample).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        wav.extend_from_slice(&pcm.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_mono_wav_header() {
        let wav = pcm16_wav(&[0.0; 16_000], 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 32_044);
    }
}
