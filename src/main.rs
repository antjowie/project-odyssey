mod camera;
mod debug;
mod world;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(camera::CameraPlugin)
        .add_plugins(debug::DebugPlugin)
        .add_plugins(world::WorldPlugin)
        .run();
}
