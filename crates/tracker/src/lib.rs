pub mod face_mesh;
pub mod hand;
pub mod holistic;
pub mod palm;
pub mod pose;
pub mod preprocess;

use glam::{Vec2, Vec3};

/// Combined tracking result from all models.
#[derive(Debug, Clone)]
pub struct HolisticResult {
    pub face_landmarks: Option<Vec<Vec3>>,
    pub pose_landmarks_3d: Option<Vec<Vec3>>,
    pub pose_landmarks_2d: Option<Vec<Vec2>>,
    pub left_hand_landmarks: Option<Vec<Vec3>>,
    pub right_hand_landmarks: Option<Vec<Vec3>>,
}
