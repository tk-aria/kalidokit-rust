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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_missing_extension() {
        let e = VrmError::MissingExtension("VRM".into());
        assert_eq!(e.to_string(), "VRM extension missing: VRM");
    }

    #[test]
    fn display_invalid_bone() {
        let e = VrmError::InvalidBone("badBone".into());
        assert_eq!(e.to_string(), "Invalid bone: badBone");
    }

    #[test]
    fn display_missing_data() {
        let e = VrmError::MissingData("blob".into());
        assert_eq!(e.to_string(), "Missing data: blob");
    }

    #[test]
    fn from_json_error() {
        let json_err: Result<serde_json::Value, _> = serde_json::from_str("{invalid");
        let vrm_err: VrmError = json_err.unwrap_err().into();
        assert!(matches!(vrm_err, VrmError::JsonError(_)));
    }
}
