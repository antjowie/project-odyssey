use bevy::prelude::*;

use crate::game::NetOwner;

use super::*;

#[derive(Resource)]
pub struct RailAsset {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub fn create_rail_asset(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> RailAsset {
    RailAsset {
        mesh: meshes.add(Cuboid::from_length(2.0)),
        material: materials.add(Color::BLACK),
    }
}

#[derive(Bundle, Default)]
pub struct RailBundle {
    pub pbr: PbrBundle,
    pub building: Building,
}

#[derive(Default)]
pub struct SpawnRail {
    pub is_preview: bool,
    pub transform: Transform,
}

impl Command for SpawnRail {
    fn apply(self, world: &mut World) {
        let assets = world.get_resource::<RailAsset>();

        if let Some(assets) = assets {
            let mut ec = world.spawn((
                RailBundle {
                    pbr: PbrBundle {
                        mesh: assets.mesh.clone(),
                        material: assets.material.clone(),
                        transform: self.transform,
                        ..default()
                    },
                    ..default()
                },
                NetOwner,
            ));

            if self.is_preview {
                ec.insert(BuildingPreview::default());
            }
        }
    }
}

pub fn on_place_rail(mut c: Commands, q: Query<(Entity, &Transform), Added<PlaceBuildingPreview>>) {
    q.into_iter().for_each(|(e, t)| {
        c.add(SpawnRail {
            transform: t.clone(),
            ..default()
        });
        c.entity(e).remove::<PlaceBuildingPreview>();
    });
}
