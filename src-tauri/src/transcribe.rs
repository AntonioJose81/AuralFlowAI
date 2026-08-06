use std::path::Path;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::{AuralFlowError, Result};

pub fn transcribe(model_path: &Path, samples: &[f32], language: &str) -> Result<String> {
    if !model_path.exists() {
        return Err(AuralFlowError::Model(format!(
            "no se encontró el modelo en {}",
            model_path.display()
        )));
    }
    if samples.len() < 4_800 {
        return Err(AuralFlowError::Transcription(
            "la grabación es demasiado corta".into(),
        ));
    }

    let rms = (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt();
    if rms < 0.003 {
        return Err(AuralFlowError::Transcription(
            "no se ha detectado voz; revisa el micrófono".into(),
        ));
    }

    let path = model_path
        .to_str()
        .ok_or_else(|| AuralFlowError::Model("la ruta del modelo no es UTF-8".into()))?;
    let mut context_params = WhisperContextParameters::default();
    context_params.use_gpu(cfg!(target_os = "macos"));
    context_params.flash_attn(cfg!(target_os = "macos"));
    let context = WhisperContext::new_with_params(path, context_params)
        .map_err(|error| AuralFlowError::Model(error.to_string()))?;
    let mut state = context
        .create_state()
        .map_err(|error| AuralFlowError::Transcription(error.to_string()))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 3 });
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_no_timestamps(true);
    let threads = std::thread::available_parallelism()
        .map(|count| count.get().min(8) as i32)
        .unwrap_or(4);
    params.set_n_threads(threads);
    if language == "auto" {
        params.set_language(None);
    } else {
        params.set_language(Some(language));
    }

    state
        .full(params, samples)
        .map_err(|error| AuralFlowError::Transcription(error.to_string()))?;

    let text = state
        .as_iter()
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return Err(AuralFlowError::Transcription(
            "Whisper no devolvió texto".into(),
        ));
    }
    Ok(cleaned)
}
