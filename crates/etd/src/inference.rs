// ONNX Runtime inference session for End-of-Turn Detection.

use ndarray::Array3;
use ort::session::Session;
use ort::value::TensorRef;

use crate::error::EtdError;

/// Wraps an ORT session loaded from the smart-turn v3 ONNX model.
pub struct EtdSession {
    session: Session,
}

impl EtdSession {
    /// Load the ONNX model from the given path.
    ///
    /// Uses a single intra-op thread for CPU inference.
    pub fn load(model_path: &std::path::Path) -> Result<Self, EtdError> {
        let session = Session::builder()
            .map_err(|e| EtdError::ModelLoad(format!("failed to create session builder: {e}")))?
            .with_intra_threads(1)
            .map_err(|e| EtdError::ModelLoad(format!("failed to set intra threads: {e}")))?
            .commit_from_file(model_path)
            .map_err(|e| {
                EtdError::ModelLoad(format!(
                    "failed to load model from {}: {e}",
                    model_path.display()
                ))
            })?;

        Ok(Self { session })
    }

    /// Run inference on mel-spectrogram features.
    ///
    /// `mel_features` must contain exactly `n_mels * n_frames` floats in row-major order.
    /// Returns a probability in `[0.0, 1.0]` after applying sigmoid to the raw logit.
    pub fn infer(
        &mut self,
        mel_features: &[f32],
        n_mels: usize,
        n_frames: usize,
    ) -> Result<f32, EtdError> {
        let expected_len = n_mels * n_frames;
        if mel_features.len() != expected_len {
            return Err(EtdError::Inference(format!(
                "expected {} mel features ({} x {}), got {}",
                expected_len,
                n_mels,
                n_frames,
                mel_features.len()
            )));
        }

        // Create Array3 with shape (1, n_mels, n_frames) from the flat slice.
        let input_array = Array3::from_shape_vec((1, n_mels, n_frames), mel_features.to_vec())
            .map_err(|e| EtdError::Inference(format!("failed to create input tensor: {e}")))?;

        let input_ref = TensorRef::from_array_view(&input_array)
            .map_err(|e| EtdError::Inference(format!("failed to create tensor ref: {e}")))?;

        let inputs = ort::inputs!["input_features" => input_ref];

        let outputs = self
            .session
            .run(inputs)
            .map_err(|e| EtdError::Inference(format!("inference failed: {e}")))?;

        // Extract the logit from output tensor shape (batch, 1).
        let (_shape, logits) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| EtdError::Inference(format!("failed to extract output tensor: {e}")))?;

        let logit = logits.first().copied().unwrap_or(0.0);

        // Apply sigmoid to convert logit to probability.
        let probability = 1.0 / (1.0 + (-logit).exp());

        Ok(probability)
    }
}
