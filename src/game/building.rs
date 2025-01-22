//! Any buildings that can be built and placed
use super::*;
use bevy::{ecs::traversal::Traversal, pbr::NotShadowCaster};

use rail::*;
pub mod rail;

pub(super) fn building_plugin(app: &mut App) {
    app.add_systems(Startup, load_assets);
    app.add_plugins(rail_plugin);

    app.add_systems(
        Update,
        cleanup_build_preview_on_state_change.run_if(on_event::<PlayerStateEvent>),
    );
    // app.add_systems(
    //     Update,
    //     (
    //         on_add_build_preview_component,
    //         update_build_preview_material,
    //     )
    //         .chain(),
    // );
    // app.add_systems(PostUpdate, on_remove_build_preview_component);
}

#[derive(Resource, PartialEq)]
pub struct BuildingPreviewMaterial {
    valid: Handle<StandardMaterial>,
    invalid: Handle<StandardMaterial>,
    // preview: Handle<StandardMaterial>,
}

fn load_assets(
    mut c: Commands,
    meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    c.insert_resource(BuildingPreviewMaterial {
        valid: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 1.0, 0.2, 0.5),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        invalid: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.2, 0.2, 0.5),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

#[derive(Component, Default)]
pub struct Building;

#[derive(Component)]
pub struct BuildingPreview {
    state_instigator: Entity,
    orig_material: MeshMaterial3d<StandardMaterial>,
    pub valid: bool,
}

impl BuildingPreview {
    fn new(state_instigator: Entity) -> BuildingPreview {
        BuildingPreview {
            state_instigator,
            orig_material: MeshMaterial3d::<StandardMaterial>::default(),
            valid: false,
        }
    }
}

impl Traversal for &BuildingPreview {
    fn traverse(item: Self::Item<'_>) -> Option<Entity> {
        Some(item.state_instigator)
    }
}

fn cleanup_build_preview_on_state_change(
    mut c: Commands,
    q: Query<Entity, With<BuildingPreview>>,
    mut event: EventReader<PlayerStateEvent>,
) {
    {
        for e in event.read() {
            if e.new_state == PlayerState::Viewing && e.old_state != PlayerState::Viewing {
                q.into_iter().for_each(|e| {
                    c.entity(e).despawn();
                });
            }
        }
    }
}

fn on_add_build_preview_component(
    mut c: Commands,
    mut q: Query<
        (
            Entity,
            &MeshMaterial3d<StandardMaterial>,
            &mut BuildingPreview,
        ),
        (With<Building>, Added<BuildingPreview>),
    >,
) {
    q.iter_mut().for_each(|(e, handle, mut preview)| {
        c.entity(e).insert(NotShadowCaster);
        preview.orig_material = handle.clone();
    });
}

fn on_remove_build_preview_component(
    mut c: Commands,
    mut q: Query<(&mut MeshMaterial3d<StandardMaterial>, &BuildingPreview), With<Building>>,
    mut removed: RemovedComponents<BuildingPreview>,
) {
    for entity in removed.read() {
        if let Ok((mut handle, preview)) = q.get_mut(entity) {
            c.entity(entity).remove::<NotShadowCaster>();
            *handle = preview.orig_material.clone();
        }
    }
}

fn update_build_preview_material(
    mut q: Query<(&mut MeshMaterial3d<StandardMaterial>, &BuildingPreview)>,
    preview_material: Res<BuildingPreviewMaterial>,
) {
    q.iter_mut().for_each(|(mut mat, preview)| {
        if preview.valid && mat.0 != preview_material.valid {
            mat.0 = preview_material.valid.clone();
        } else if !preview.valid && mat.0 != preview_material.invalid {
            mat.0 = preview_material.invalid.clone();
        };
    });
}
