mod camera;
mod debug;
mod world;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(
            // here we configure the main window
            WindowPlugin {
                primary_window: Some(Window {
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            },
        ))
        .add_plugins(camera::CameraPlugin)
        .add_plugins(debug::DebugPlugin)
        .add_plugins(world::WorldPlugin)
        .run();
}
