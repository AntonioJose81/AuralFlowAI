mod audio;
mod config;
mod credentials;
mod error;
mod model;
mod online;
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
    groq_api_key: Mutex<Option<String>>,
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

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn groq_api_key(state: &RuntimeState) -> std::result::Result<Option<String>, String> {
    let mut cached = state
        .groq_api_key
        .lock()
        .map_err(|_| "Clave API: estado bloqueado".to_string())?;
    if cached.is_none() {
        *cached = credentials::load_groq_key()?;
    }
    Ok(cached.clone())
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
fn set_groq_api_key(
    state: State<'_, RuntimeState>,
    api_key: String,
) -> std::result::Result<(), String> {
    let key = api_key.trim();
    if key.len() < 20 || key.len() > 300 {
        return Err("La clave API de Groq no parece válida.".into());
    }
    credentials::save_groq_key(key)?;
    *state
        .groq_api_key
        .lock()
        .map_err(|_| "Clave API: estado bloqueado".to_string())? = Some(key.to_owned());
    Ok(())
}

#[tauri::command]
fn groq_key_status(state: State<'_, RuntimeState>) -> std::result::Result<bool, String> {
    Ok(groq_api_key(&state)?.is_some())
}

#[tauri::command]
fn clear_groq_api_key(state: State<'_, RuntimeState>) -> std::result::Result<(), String> {
    credentials::delete_groq_key()?;
    *state
        .groq_api_key
        .lock()
        .map_err(|_| "Clave API: estado bloqueado".to_string())? = None;
    Ok(())
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
    let settings = config::load(&app).map_err(|error| error.to_string())?;
    let preview_seconds = if settings.engine == "groq" { 9 } else { 12 };
    let audio = state
        .recorder
        .lock()
        .map_err(|_| "Audio: estado bloqueado".to_string())?
        .snapshot_recent(preview_seconds)
        .map_err(|error| error.to_string())?;
    if audio.samples.len() < 16_000 {
        return Ok(PreviewResult {
            text: String::new(),
        });
    }

    if settings.engine == "groq" {
        let api_key = groq_api_key(&state)?
            .ok_or_else(|| "Servicio online: falta la clave API de Groq".to_string())?;
        let text = online::transcribe_groq(&audio.samples, &settings.language, &api_key)
            .await
            .map_err(|error| error.to_string())?;
        return Ok(PreviewResult { text });
    }

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
    let language = settings.language.clone();
    let auto_paste = settings.auto_paste;
    let started = Instant::now();

    if settings.engine == "groq" {
        let api_key = groq_api_key(&state)?
            .ok_or_else(|| "Servicio online: falta la clave API de Groq".to_string())?;
        let text = online::transcribe_groq(&audio.samples, &language, &api_key)
            .await
            .map_err(|error| error.to_string())?;
        let paste_text = text.clone();
        let (pasted, warning) = if auto_paste {
            tauri::async_runtime::spawn_blocking(move || match paste::paste_text(&paste_text) {
                Ok(()) => (true, None),
                Err(error) => (
                    false,
                    Some(format!("El texto se transcribió, pero no se pudo pegar: {error}")),
                ),
            })
            .await
            .map_err(|error| format!("Portapapeles: el proceso terminó inesperadamente: {error}"))?
        } else {
            (false, None)
        };
        return Ok(TranscriptResult {
            text,
            elapsed_ms: started.elapsed().as_millis(),
            pasted,
            warning,
        });
    }

    let model_path = model::model_path(&app, &settings.model_name).map_err(|error| error.to_string())?;
    let transcriber = state.transcriber.clone();

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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(RuntimeState::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let window_to_hide = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_to_hide.hide();
                    }
                });
            }
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
                    show_main_window(app);
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
                    show_main_window(app);
                }
            })
            .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            set_groq_api_key,
            groq_key_status,
            clear_groq_api_key,
            model_status,
            download_model,
            prepare_model,
            start_recording,
            preview_transcription,
            stop_and_transcribe,
            copy_text,
        ])
        .build(tauri::generate_context!())
        .expect("error al construir AuralFlow")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main_window(app);
            }
        });
}
