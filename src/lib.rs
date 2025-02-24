//! Entrypoint of the app
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_rand::{plugin::EntropyPlugin, prelude::WyRand};

pub mod debug;
pub mod game;
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
            EntropyPlugin::<WyRand>::default(),
            PhysicsPlugins::default(),
            EguiPlugin,
            MeshPickingPlugin,
            debug::DebugPlugin,
            game::GamePlugin,
        ));
    }
}
