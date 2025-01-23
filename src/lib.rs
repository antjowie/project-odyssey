//! Entrypoint of the app
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;

pub mod camera;
pub mod debug;
pub mod game;
pub mod input;
pub mod spline;
pub mod util;

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
            PhysicsPlugins::default(),
            EguiPlugin,
            MeshPickingPlugin,
            camera::CameraPlugin,
            debug::DebugPlugin,
            game::GamePlugin,
            spline::SplinePlugin,
        ));
    }
}
