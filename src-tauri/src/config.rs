use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AuralFlowError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub model_name: String,
    pub language: String,
    pub hotkey: String,
    pub auto_paste: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model_name: "small-q5_1".into(),
            language: "es".into(),
            hotkey: "CommandOrControl+Shift+Space".into(),
            auto_paste: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !crate::model::is_supported_model(&self.model_name) {
            return Err(AuralFlowError::Config("modelo no permitido".into()));
        }
        if !matches!(self.language.as_str(), "es" | "en" | "auto") {
            return Err(AuralFlowError::Config("idioma no permitido".into()));
        }
        if self.hotkey.trim().is_empty() || self.hotkey.len() > 100 {
            return Err(AuralFlowError::Config("el atajo global no es válido".into()));
        }
        Ok(())
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| AuralFlowError::Config(error.to_string()))?;
    Ok(directory.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Result<Settings> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| AuralFlowError::Config(format!("no se pudo leer {}: {error}", path.display())))?;
    let settings: Settings = serde_json::from_str(&contents)
        .map_err(|error| AuralFlowError::Config(format!("JSON inválido: {error}")))?;
    settings.validate()?;
    Ok(settings)
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<()> {
    settings.validate()?;
    let path = settings_path(app)?;
    let directory = path
        .parent()
        .ok_or_else(|| AuralFlowError::Config("ruta de configuración inválida".into()))?;
    fs::create_dir_all(directory)
        .map_err(|error| AuralFlowError::Config(format!("no se pudo crear la carpeta: {error}")))?;

    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| AuralFlowError::Config(error.to_string()))?;
    fs::write(&path, contents)
        .map_err(|error| AuralFlowError::Config(format!("no se pudo guardar: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn rejects_unknown_models() {
        let mut settings = Settings::default();
        settings.model_name = "modelo-inventado".into();
        assert!(settings.validate().is_err());
    }
}
