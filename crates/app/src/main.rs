use bevy::prelude::*;

mod components;
mod plugins;
mod systems;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KalidoKit Rust - VRM Motion Capture".to_string(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(plugins::camera::CameraCapturePlugin)
        .add_plugins(plugins::tracker::TrackerPlugin)
        .add_plugins(plugins::avatar::AvatarPlugin)
        .run();
}
