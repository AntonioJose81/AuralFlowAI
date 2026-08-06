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
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State,
};

#[derive(Default)]
struct RuntimeState {
    recorder: Mutex<AudioRecorder>,
    transcriber: transcribe::Transcriber,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptResult {
    text: String,
    elapsed_ms: u128,
    pasted: bool,
    warning: Option<String>,
}

#[derive(Debug, Serialize)]
struct PreviewResult {
    text: String,
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
async fn prepare_model(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    model_name: String,
) -> std::result::Result<(), String> {
    let model_path = model::model_path(&app, &model_name).map_err(|error| error.to_string())?;
    let transcriber = state.transcriber.clone();
    tauri::async_runtime::spawn_blocking(move || transcriber.prepare(&model_path))
        .await
        .map_err(|error| format!("Modelo: la preparación terminó inesperadamente: {error}"))?
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
async fn preview_transcription(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> std::result::Result<PreviewResult, String> {
    let audio = state
        .recorder
        .lock()
        .map_err(|_| "Audio: estado bloqueado".to_string())?
        .snapshot_recent(12)
        .map_err(|error| error.to_string())?;
    if audio.samples.len() < 16_000 {
        return Ok(PreviewResult {
            text: String::new(),
        });
    }

    let settings = config::load(&app).map_err(|error| error.to_string())?;
    let model_path = model::model_path(&app, &settings.model_name).map_err(|error| error.to_string())?;
    let language = settings.language;
    let transcriber = state.transcriber.clone();
    tauri::async_runtime::spawn_blocking(move || {
        match transcriber.transcribe(&model_path, &audio.samples, &language, true) {
            Ok(text) => Ok(PreviewResult { text }),
            Err(error::AuralFlowError::Transcription(_)) => Ok(PreviewResult {
                text: String::new(),
            }),
            Err(error) => Err(error.to_string()),
        }
    })
    .await
    .map_err(|error| format!("Transcripción: la vista previa terminó inesperadamente: {error}"))?
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
    let transcriber = state.transcriber.clone();
    let started = Instant::now();

    tauri::async_runtime::spawn_blocking(move || {
        let text = transcriber
            .transcribe(&model_path, &audio.samples, &language, false)
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
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Mostrar AuralFlow", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("AuralFlow · dictado local")
                .menu(&menu)
                .show_menu_on_left_click(false);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            })
            .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            model_status,
            download_model,
            prepare_model,
            start_recording,
            preview_transcription,
            stop_and_transcribe,
            copy_text,
        ])
        .run(tauri::generate_context!())
        .expect("error al ejecutar AuralFlow");
}
