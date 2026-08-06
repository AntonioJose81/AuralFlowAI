mod audio;
mod config;
mod error;
mod model;
mod paste;
mod transcribe;

use std::{sync::Mutex, time::Instant};

use audio::{AudioInfo, AudioRecorder};
use config::Settings;
use model::ModelStatus;
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Default)]
struct RuntimeState {
    recorder: Mutex<AudioRecorder>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptResult {
    text: String,
    elapsed_ms: u128,
    pasted: bool,
    warning: Option<String>,
}

#[tauri::command]
fn load_settings(app: AppHandle) -> std::result::Result<Settings, String> {
    config::load(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> std::result::Result<(), String> {
    config::save(&app, &settings).map_err(|error| error.to_string())
}

#[tauri::command]
fn model_status(app: AppHandle, model_name: String) -> std::result::Result<ModelStatus, String> {
    model::status(&app, &model_name).map_err(|error| error.to_string())
}

#[tauri::command]
async fn download_model(
    app: AppHandle,
    model_name: String,
) -> std::result::Result<ModelStatus, String> {
    model::download(&app, &model_name)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_recording(state: State<'_, RuntimeState>) -> std::result::Result<AudioInfo, String> {
    state
        .recorder
        .lock()
        .map_err(|_| "Audio: estado bloqueado".to_string())?
        .start()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn stop_and_transcribe(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> std::result::Result<TranscriptResult, String> {
    let audio = state
        .recorder
        .lock()
        .map_err(|_| "Audio: estado bloqueado".to_string())?
        .stop()
        .map_err(|error| error.to_string())?;
    let settings = config::load(&app).map_err(|error| error.to_string())?;
    let model_path = model::model_path(&app, &settings.model_name).map_err(|error| error.to_string())?;
    let language = settings.language.clone();
    let auto_paste = settings.auto_paste;
    let started = Instant::now();

    tauri::async_runtime::spawn_blocking(move || {
        let text = transcribe::transcribe(&model_path, &audio.samples, &language)
            .map_err(|error| error.to_string())?;
        let (pasted, warning) = if auto_paste {
            match paste::paste_text(&text) {
                Ok(()) => (true, None),
                Err(error) => (
                    false,
                    Some(format!("El texto se transcribió, pero no se pudo pegar: {error}")),
                ),
            }
        } else {
            (false, None)
        };
        Ok(TranscriptResult {
            text,
            elapsed_ms: started.elapsed().as_millis(),
            pasted,
            warning,
        })
    })
    .await
    .map_err(|error| format!("Transcripción: el proceso terminó inesperadamente: {error}"))?
}

#[tauri::command]
fn copy_text(text: String) -> std::result::Result<(), String> {
    paste::copy_text(&text).map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(RuntimeState::default())
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            model_status,
            download_model,
            start_recording,
            stop_and_transcribe,
            copy_text,
        ])
        .run(tauri::generate_context!())
        .expect("error al ejecutar AuralFlow");
}
