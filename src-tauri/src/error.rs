use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuralFlowError {
    #[error("Audio: {0}")]
    Audio(String),
    #[error("Modelo: {0}")]
    Model(String),
    #[error("Transcripción: {0}")]
    Transcription(String),
    #[error("Servicio online: {0}")]
    Online(String),
    #[error("Configuración: {0}")]
    Config(String),
    #[error("Portapapeles: {0}")]
    Clipboard(String),
}

pub type Result<T> = std::result::Result<T, AuralFlowError>;
