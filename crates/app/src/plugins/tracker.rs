use bevy::prelude::*;

pub struct TrackerPlugin;

impl Plugin for TrackerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_tracker)
            .add_systems(Update, run_inference);
    }
}

fn setup_tracker(mut _commands: Commands) {
    // Load ONNX models and create HolisticTracker resource
    todo!("Initialize ONNX tracker sessions")
}

fn run_inference() {
    // Run face/pose/hand inference on captured frame
    todo!("Run ML inference pipeline")
}
