use bevy::prelude::*;

pub struct CameraCapturePlugin;

impl Plugin for CameraCapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera)
            .add_systems(Update, capture_frame);
    }
}

fn setup_camera(mut _commands: Commands) {
    // Initialize webcam via nokhwa
    todo!("Initialize webcam capture")
}

fn capture_frame() {
    // Grab frame from webcam each tick
    todo!("Capture webcam frame")
}
