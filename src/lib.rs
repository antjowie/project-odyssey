//! Entrypoint of the app
use bevy::prelude::*;

pub mod camera;
pub mod debug;
pub mod game;
pub mod input;

/// Plugin that represents the game
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
