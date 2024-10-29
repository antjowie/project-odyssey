use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::camera;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_world);
    }

    fn name(&self) -> &str {
        "WorldPlugin"
    }
}

fn setup_world(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    c.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(100.0))),
        material: materials.add(Color::WHITE),
        ..default()
    });
    c.spawn(camera::PanOrbitCameraBundle {
        input: InputManagerBundle::with_map(camera::CameraAction::default_player_mapping()),
        ..default()
    });
}
