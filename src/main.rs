mod building;
mod camera;
mod debug;
mod game;
mod world;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(
                // here we configure the main window
                WindowPlugin {
                    primary_window: Some(Window {
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                },
            ),
            building::BuildingPlugin,
            camera::CameraPlugin,
            debug::DebugPlugin,
            game::GamePlugin,
            world::WorldPlugin,
        ))
        .run();
}
