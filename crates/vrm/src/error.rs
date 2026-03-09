#[derive(Debug, thiserror::Error)]
pub enum VrmError {
    #[error("glTF parse error: {0}")]
    GltfError(#[from] gltf::Error),
    #[error("VRM extension missing: {0}")]
    MissingExtension(String),
    #[error("Invalid bone: {0}")]
    InvalidBone(String),
    #[error("Missing data: {0}")]
    MissingData(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
