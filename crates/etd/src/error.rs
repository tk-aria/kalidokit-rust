use thiserror::Error;

#[derive(Debug, Error)]
pub enum EtdError {
    #[error("model load failed: {0}")]
    ModelLoad(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("invalid audio: {0}")]
    InvalidAudio(String),
}
