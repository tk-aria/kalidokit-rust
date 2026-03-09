use bevy::prelude::*;
use crate::components::rig::RigTarget;

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_avatar)
            .add_systems(Update, apply_rig_to_vrm);
    }
}

fn setup_avatar(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Load VRM avatar
    // Note: VRM model needs Y-axis 180deg rotation to face camera
    let _ = (&mut commands, &asset_server);
    todo!("Load VRM avatar and add to scene")
}

fn apply_rig_to_vrm(
    _rig_data: Option<Res<crate::components::rig::CurrentRig>>,
    mut _transforms: Query<&mut Transform, With<RigTarget>>,
) {
    // Apply solver results to VRM bones with slerp/lerp interpolation
    // Dampener and lerp amounts per bone:
    //   Neck: dampener=0.7, lerp=0.3
    //   Hips rotation: dampener=0.7, lerp=0.3
    //   Hips position: dampener=1.0, lerp=0.07
    //   Chest: dampener=0.25, lerp=0.3
    //   Spine: dampener=0.45, lerp=0.3
    //   Limbs: dampener=1.0, lerp=0.3
    todo!("Apply rig data to VRM bones")
}
