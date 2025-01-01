//! Program entry point
//! The game is compiled as a library to easily integrate a dedicated test project
use bevy::prelude::*;
use project_odyssey::AppPlugin;

fn main() {
    App::new().add_plugins(AppPlugin).run();
}
