use bevy::prelude::*;
use solver::{RiggedFace, RiggedHand, RiggedPose};

/// Marker component for the VRM avatar entity.
#[derive(Component)]
pub struct RigTarget;

/// Stores the latest rig solver results.
#[derive(Resource, Default)]
pub struct CurrentRig {
    pub face: Option<RiggedFace>,
    pub pose: Option<RiggedPose>,
    pub left_hand: Option<RiggedHand>,
    pub right_hand: Option<RiggedHand>,
}
