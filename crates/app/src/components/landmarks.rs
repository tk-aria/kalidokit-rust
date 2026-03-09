use bevy::prelude::*;
use glam::{Vec2, Vec3};

/// Stores the latest landmarks from ML inference.
#[derive(Resource, Default)]
pub struct CurrentLandmarks {
    pub face: Option<Vec<Vec3>>,
    pub pose: Option<(Vec<Vec3>, Vec<Vec2>)>,
    pub left_hand: Option<Vec<Vec3>>,
    pub right_hand: Option<Vec<Vec3>>,
}
