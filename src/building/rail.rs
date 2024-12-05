use bevy::{prelude::*, render::extract_component::ExtractComponent};

use crate::game::{NetOwner, PlayerCursor};

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

/// Contains the details to build and connect a rail
#[derive(Component, Default)]
pub struct RailPathState {
    start_joint: RailPathJoint,
    end_joint: RailPathJoint,
}

#[derive(Default)]
pub struct RailPathJoint {
    pos: Vec3,
    left: Option<Entity>,
    straight: Option<Entity>,
    right: Option<Entity>,
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

enum RailPathPlacementType {
    PlaceStart,
    PlaceEnd,
    Expand(RailPathJoint),
}

pub fn on_place_rail(
    mut c: Commands,
    q: Query<(Entity, &Transform), Added<PlaceBuildingPreview>>,
    cursor: Query<PlayerCursor, With<NetOwner>>,
    has_placed_start: Local<bool>,
) {
    q.into_iter().for_each(|(e, t)| {
        c.add(SpawnRail {
            transform: t.clone(),
            ..default()
        });
        c.entity(e).remove::<PlaceBuildingPreview>();
    });
}
