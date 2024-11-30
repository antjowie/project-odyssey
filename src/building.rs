use bevy::prelude::*;

pub mod rail;

pub struct BuildingPlugin;

#[derive(Resource)]
struct RailAsset {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_assets);
    }
}

fn load_assets(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    c.insert_resource(RailAsset {
        mesh: meshes.add(Cuboid::from_length(2.0)),
        material: materials.add(Color::BLACK),
    });
}
