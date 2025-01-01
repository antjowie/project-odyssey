//! Entrypoint of the app
use bevy::prelude::*;

mod camera;
mod debug;
mod game;
mod input;

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
            camera::CameraPlugin,
            debug::DebugPlugin,
            game::GamePlugin,
        ));
    }
}
