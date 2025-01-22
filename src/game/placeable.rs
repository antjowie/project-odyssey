//! Any placeable are things that can be placed
use super::*;
use bevy::{ecs::traversal::Traversal, pbr::NotShadowCaster};

use rail::*;
use train::*;
pub mod rail;
pub mod train;

#[derive(Component, Default, PartialEq, Clone)]
pub enum Placeable {
    #[default]
    Rail,
    Train,
}

pub fn is_placeable(placeable: Placeable) -> impl FnMut(Query<&Placeable>) -> bool {
    move |query: Query<&Placeable>| !query.is_empty() && *query.single() == placeable
}

pub(super) fn placeable_plugin(app: &mut App) {
    app.add_systems(Startup, load_assets);
    app.add_plugins(rail_plugin);
    app.add_plugins(train_plugin);

    app.add_systems(
        Update,
        cleanup_build_preview_on_state_change.run_if(on_event::<PlayerStateEvent>),
    );
    app.add_systems(
        Update,
        (
            on_add_build_preview_component,
            update_build_preview_material,
        )
            .chain(),
    );
    app.add_systems(PostUpdate, on_remove_build_preview_component);
}

#[derive(Resource, PartialEq)]
pub struct PlaceablePreviewMaterial {
    valid: Handle<StandardMaterial>,
    invalid: Handle<StandardMaterial>,
    preview: Handle<StandardMaterial>,
}

fn load_assets(mut c: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    c.insert_resource(PlaceablePreviewMaterial {
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
        preview: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.2, 1.0, 0.5),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

#[derive(Component)]
pub struct PlaceablePreview {
    /// Represents the PlayerState that spawned this
    state_instigator: Entity,
    orig_material: MeshMaterial3d<StandardMaterial>,
    pub valid: bool,
}

impl PlaceablePreview {
    fn new(state_instigator: Entity) -> PlaceablePreview {
        PlaceablePreview {
            state_instigator,
            orig_material: MeshMaterial3d::<StandardMaterial>::default(),
            valid: false,
        }
    }
}

/// When we push a cancel event, we push it to the Placeable so we can bubble it up to player (and handle build cancel)
impl Traversal for &PlaceablePreview {
    fn traverse(item: Self::Item<'_>) -> Option<Entity> {
        Some(item.state_instigator)
    }
}

fn cleanup_build_preview_on_state_change(
    mut c: Commands,
    q: Query<Entity, With<PlaceablePreview>>,
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
            &mut PlaceablePreview,
        ),
        (With<Placeable>, Added<PlaceablePreview>),
    >,
) {
    q.iter_mut().for_each(|(e, handle, mut preview)| {
        c.entity(e).insert(NotShadowCaster);
        preview.orig_material = handle.clone();
    });
}

fn on_remove_build_preview_component(
    mut c: Commands,
    mut q: Query<(&mut MeshMaterial3d<StandardMaterial>, &PlaceablePreview), With<Placeable>>,
    mut removed: RemovedComponents<PlaceablePreview>,
) {
    for entity in removed.read() {
        if let Ok((mut handle, preview)) = q.get_mut(entity) {
            c.entity(entity).remove::<NotShadowCaster>();
            *handle = preview.orig_material.clone();
        }
    }
}

fn update_build_preview_material(
    mut q: Query<(&mut MeshMaterial3d<StandardMaterial>, &PlaceablePreview)>,
    preview_material: Res<PlaceablePreviewMaterial>,
) {
    q.iter_mut().for_each(|(mut mat, preview)| {
        if preview.valid && mat.0 != preview_material.valid {
            mat.0 = preview_material.valid.clone();
        } else if !preview.valid && mat.0 != preview_material.invalid {
            mat.0 = preview_material.invalid.clone();
        };
    });
}
