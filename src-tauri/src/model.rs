use std::path::PathBuf;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::error::{AuralFlowError, Result};

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const SUPPORTED_MODELS: &[&str] = &["base-q5_1", "small-q5_1", "large-v3-turbo-q5_0"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub model_name: String,
    pub installed: bool,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

pub fn is_supported_model(name: &str) -> bool {
    SUPPORTED_MODELS.contains(&name)
}

pub fn model_path(app: &AppHandle, name: &str) -> Result<PathBuf> {
    if !is_supported_model(name) {
        return Err(AuralFlowError::Model("modelo no permitido".into()));
    }
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| AuralFlowError::Model(error.to_string()))?
        .join("models");
    Ok(directory.join(format!("ggml-{name}.bin")))
}

pub fn status(app: &AppHandle, name: &str) -> Result<ModelStatus> {
    let path = model_path(app, name)?;
    let installed = path
        .metadata()
        .map(|metadata| metadata.len() > 1_000_000)
        .unwrap_or(false);
    Ok(ModelStatus {
        model_name: name.into(),
        installed,
        path: path.display().to_string(),
    })
}

pub async fn download(app: &AppHandle, name: &str) -> Result<ModelStatus> {
    let path = model_path(app, name)?;
    let directory = path
        .parent()
        .ok_or_else(|| AuralFlowError::Model("ruta de modelos inválida".into()))?;
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|error| AuralFlowError::Model(error.to_string()))?;

    let url = format!("{MODEL_BASE_URL}/ggml-{name}.bin");
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|error| AuralFlowError::Model(format!("falló la descarga: {error}")))?
        .error_for_status()
        .map_err(|error| AuralFlowError::Model(format!("el servidor rechazó la descarga: {error}")))?;
    let total_bytes = response.content_length();
    let temporary = path.with_extension("bin.part");
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&temporary)
        .await
        .map_err(|error| AuralFlowError::Model(error.to_string()))?;
    let mut stream = response.bytes_stream();
    let mut downloaded_bytes = 0_u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| AuralFlowError::Model(error.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|error| AuralFlowError::Model(error.to_string()))?;
        downloaded_bytes += chunk.len() as u64;
        let _ = app.emit(
            "model-download-progress",
            DownloadProgress {
                downloaded_bytes,
                total_bytes,
            },
        );
    }
    file.flush()
        .await
        .map_err(|error| AuralFlowError::Model(error.to_string()))?;

    if downloaded_bytes < 1_000_000 {
        drop(file);
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(AuralFlowError::Model("el archivo descargado no parece un modelo válido".into()));
    }
    file.rewind()
        .await
        .map_err(|error| AuralFlowError::Model(error.to_string()))?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)
        .await
        .map_err(|error| AuralFlowError::Model(error.to_string()))?;
    drop(file);
    if &magic != b"ggml" && &magic != b"lmgg" {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(AuralFlowError::Model(
            "la descarga no tiene el formato GGML esperado".into(),
        ));
    }
    if path.exists() {
        tokio::fs::remove_file(&path)
            .await
            .map_err(|error| AuralFlowError::Model(error.to_string()))?;
    }
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|error| AuralFlowError::Model(error.to_string()))?;
    status(app, name)
}
