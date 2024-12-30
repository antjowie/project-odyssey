use bevy::prelude::*;

mod building;
mod camera;
mod debug;
mod game;
mod input;
mod world;

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
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
            MeshPickingPlugin,
            building::BuildingPlugin,
            camera::CameraPlugin,
            debug::DebugPlugin,
            game::GamePlugin,
            world::WorldPlugin,
        ));
    }
}
