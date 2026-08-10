use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::{AuralFlowError, Result};

struct CachedModel {
    path: PathBuf,
    context: Arc<WhisperContext>,
}

#[derive(Clone, Default)]
pub struct Transcriber {
    cache: Arc<Mutex<Option<CachedModel>>>,
    inference: Arc<Mutex<()>>,
}

impl Transcriber {
    pub fn prepare(&self, model_path: &Path) -> Result<()> {
        self.context(model_path).map(|_| ())
    }

    pub fn transcribe(
        &self,
        model_path: &Path,
        samples: &[f32],
        language: &str,
        preview: bool,
    ) -> Result<String> {
        if !model_path.exists() {
            return Err(AuralFlowError::Model(format!(
                "no se encontró el modelo en {}",
                model_path.display()
            )));
        }
        let _inference = self
            .inference
            .lock()
            .map_err(|_| AuralFlowError::Transcription("motor de inferencia bloqueado".into()))?;
        transcribe_with_context(self.context(model_path)?, samples, language, preview)
    }

    fn context(&self, model_path: &Path) -> Result<Arc<WhisperContext>> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| AuralFlowError::Model("caché del modelo bloqueada".into()))?;
        if let Some(cached) = cache.as_ref() {
            if cached.path == model_path {
                return Ok(Arc::clone(&cached.context));
            }
        }

        let mut context_params = WhisperContextParameters::default();
        context_params.use_gpu(false);
        context_params.flash_attn(false);
        let context = Arc::new(
            WhisperContext::new_with_params(model_path, context_params)
                .map_err(|error| AuralFlowError::Model(error.to_string()))?,
        );
        *cache = Some(CachedModel {
            path: model_path.to_path_buf(),
            context: Arc::clone(&context),
        });
        Ok(context)
    }
}

fn transcribe_with_context(
    context: Arc<WhisperContext>,
    samples: &[f32],
    language: &str,
    preview: bool,
) -> Result<String> {
    if samples.len() < 4_800 {
        return Err(AuralFlowError::Transcription(
            "la grabación es demasiado corta".into(),
        ));
    }

    let rms =
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt();
    if rms < 0.003 {
        return Err(AuralFlowError::Transcription(
            "no se ha detectado voz; revisa el micrófono".into(),
        ));
    }

    let mut state = context
        .create_state()
        .map_err(|error| AuralFlowError::Transcription(error.to_string()))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_no_timestamps(true);
    params.set_single_segment(preview);
    params.set_no_context(preview);
    let available = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    let threads = if available > 4 {
        available - 1
    } else {
        available
    };
    let threads = threads.clamp(1, 12) as i32;
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
