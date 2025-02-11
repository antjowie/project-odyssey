//! Any placeable are things that can be placed
use super::*;
use avian3d::prelude::Collider;
use bevy::{ecs::traversal::Traversal, pbr::NotShadowCaster};

use destroyer::*;
use rail::*;
use train::*;
pub mod destroyer;
pub mod rail;
pub mod train;

pub(super) fn placeable_plugin(app: &mut App) {
    app.add_systems(Startup, load_placeable_assets);
    app.add_plugins(destroyer_plugin);
    app.add_plugins(rail_plugin);
    app.add_plugins(train_plugin);
    app.add_event::<PlaceablePreviewChangedEvent>();

    app.add_systems(
        Update,
        (
            pick_hovered_placeable,
            cleanup_build_preview_on_state_change.run_if(on_event::<PlayerStateChangedEvent>),
            update_picked_placeable.run_if(in_player_state(PlayerState::Building)),
            on_placeable_preview_changed_event.run_if(on_event::<PlaceablePreviewChangedEvent>),
            generate_collider_on_mesh_changed,
            (
                on_placeable_preview_added,
                update_placeable_preview_material,
            )
                .chain(),
        ),
    );
    app.add_systems(PostUpdate, on_placeable_preview_removed);
}

/// Represents a placeable type
/// When used on PlayerState represents desired placeable to place
#[derive(Component, Default, PartialEq, Clone)]
pub enum Placeable {
    #[default]
    Rail,
    Train,

    /// A special case, when this intention is selected whatever we click on gets removed
    Destroyer,
}

#[derive(Event)]
pub struct PlaceablePreviewChangedEvent {
    pub new: Placeable,
    pub hovered_entity: Option<Entity>,
}

#[derive(Resource, PartialEq)]
pub struct PlaceablePreviewMaterial {
    valid: Handle<StandardMaterial>,
    invalid: Handle<StandardMaterial>,
    preview: Handle<StandardMaterial>,
}

fn load_placeable_assets(mut c: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    c.insert_resource(PlaceablePreviewMaterial {
        valid: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 1.0, 0.2, 0.8),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        invalid: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.2, 0.2, 0.8),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        preview: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.2, 1.0, 0.8),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

#[derive(Component)]
pub struct PlaceablePreview {
    /// Represents the PlayerState that spawned this
    state_instigator: Entity,
    pub valid: bool,
}

impl PlaceablePreview {
    pub fn new(state_instigator: Entity) -> PlaceablePreview {
        PlaceablePreview {
            state_instigator,
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

pub fn is_placeable_preview(
    placeable: Placeable,
) -> impl FnMut(Query<&Placeable, With<PlayerState>>) -> bool {
    move |query: Query<&Placeable, With<PlayerState>>| {
        !query.is_empty() && *query.single() == placeable
    }
}

fn on_placeable_preview_changed_event(
    mut c: Commands,
    mut ev: EventReader<PlaceablePreviewChangedEvent>,
    player: Single<Entity, With<PlayerState>>,
    previews: Query<Entity, With<PlaceablePreview>>,
    placeables: Query<
        &Transform,
        (
            Without<PlayerState>,
            With<Placeable>,
            Without<PlaceablePreview>,
        ),
    >,
    train: Res<TrainAsset>,
) {
    let e_player = player.into_inner();

    previews.iter().for_each(|e| c.entity(e).try_despawn());
    for e in ev.read() {
        let t = e
            .hovered_entity
            .map(|x| placeables.get(x).unwrap().to_owned());
        let placeable = &e.new;

        match placeable {
            Placeable::Rail => {}
            Placeable::Train => {
                c.spawn((
                    Name::new("TrainPreview"),
                    PlaceablePreview::new(e_player),
                    SceneRoot(train.scene.clone()),
                    t.unwrap_or_default().with_scale(train.scale),
                ));
            }
            Placeable::Destroyer => {}
        };
    }
}

fn cleanup_build_preview_on_state_change(
    mut c: Commands,
    q: Query<Entity, With<PlaceablePreview>>,
    mut event: EventReader<PlayerStateChangedEvent>,
) {
    {
        for e in event.read() {
            if e.new_state == PlayerState::Viewing && e.old_state == PlayerState::Building {
                q.into_iter().for_each(|e| {
                    c.entity(e).despawn();
                });
            }
        }
    }
}

fn generate_collider_on_mesh_changed(
    mut c: Commands,
    q: Query<(Entity, &Mesh3d), Changed<Mesh3d>>,
    meshes: Res<Assets<Mesh>>,
) {
    q.iter().for_each(|(e, mesh)| {
        if let Some(mesh) = meshes.get(mesh) {
            if let Some(collider) = Collider::trimesh_from_mesh(&mesh) {
                // if let Some(collider) = Collider::convex_hull_from_mesh(&mesh) {
                c.entity(e).insert(collider);
            }
        }
    });
}

fn on_placeable_preview_added(
    mut c: Commands,
    q: Query<Entity, (With<Placeable>, Added<PlaceablePreview>)>,
) {
    q.iter().for_each(|e| {
        c.entity(e)
            .insert((NotShadowCaster, PickingBehavior::IGNORE));
    });
}

/// NOTE: This is a bit redundant, as currently adding a preview component is destructive
///     You can't undo adding this component, no need to restore materials
fn on_placeable_preview_removed(
    mut c: Commands,
    mut q: Query<(), With<PlaceablePreview>>,
    mut removed: RemovedComponents<PlaceablePreview>,
) {
    for entity in removed.read() {
        if let Ok(()) = q.get_mut(entity) {
            c.entity(entity).remove::<NotShadowCaster>();
            c.entity(entity).remove::<PickingBehavior>();
        }
    }
}

fn update_placeable_preview_material(
    parent: Query<(Entity, &PlaceablePreview), Changed<PlaceablePreview>>,
    children: Query<&Children>,
    mut q: Query<&mut MeshMaterial3d<StandardMaterial>>,
    preview_material: Res<PlaceablePreviewMaterial>,
) {
    parent.iter().for_each(|(e, preview)| {
        let mut handle = |entity| {
            if let Ok(mut mat) = q.get_mut(entity) {
                if preview.valid && mat.0 != preview_material.valid {
                    mat.0 = preview_material.valid.clone();
                } else if !preview.valid && mat.0 != preview_material.invalid {
                    mat.0 = preview_material.invalid.clone();
                };
            }
        };

        handle(e);
        children.iter_descendants(e).for_each(handle);
    });
}

fn update_picked_placeable(
    mut q: Query<(&ActionState<PlayerBuildAction>, &mut Placeable)>,
    mut ev: EventWriter<PlaceablePreviewChangedEvent>,
) {
    q.iter_mut().for_each(|(input, mut placeable)| {
        let current_placeable = placeable.bypass_change_detection().to_owned();
        let mut handle = |action, item| {
            if input.just_pressed(&action) && current_placeable != item {
                *placeable = item;
                ev.send(PlaceablePreviewChangedEvent {
                    new: placeable.clone(),
                    hovered_entity: None,
                });
            }
        };
        handle(PlayerBuildAction::PickRail, Placeable::Rail);
        handle(PlayerBuildAction::PickTrain, Placeable::Train);
        handle(PlayerBuildAction::PickDestroy, Placeable::Destroyer);
    });
}

fn pick_hovered_placeable(
    mut c: Commands,
    player: Single<(
        Entity,
        &mut Placeable,
        &mut PlayerState,
        &PlayerCursor,
        Option<&ActionState<PlayerViewAction>>,
        Option<&ActionState<PlayerBuildAction>>,
    )>,
    placeables: Query<&Placeable, (Without<PlayerState>, Without<PlaceablePreview>)>,
    mut ray_cast: MeshRayCast,
    mut ev_state: EventWriter<PlayerStateChangedEvent>,
    mut ev_placeable: EventWriter<PlaceablePreviewChangedEvent>,
) {
    let (e, mut placeable, mut state, cursor, view, build) = player.into_inner();
    let just_pressed = view.is_some_and(|x| x.just_pressed(&PlayerViewAction::PickHovered))
        || build.is_some_and(|x| x.just_pressed(&PlayerBuildAction::PickHovered));

    if just_pressed {
        let hits = ray_cast.cast_ray(
            cursor.ray,
            &RayCastSettings::default()
                .always_early_exit()
                .with_filter(&|x| placeables.contains(x)),
        );

        if hits.len() > 0 {
            let new_placeable = placeables.get(hits[0].0).unwrap().to_owned();
            state.set(PlayerState::Building, &mut c, e, &mut ev_state);
            *placeable = new_placeable;
            ev_placeable.send(PlaceablePreviewChangedEvent {
                new: placeable.clone(),
                hovered_entity: Some(hits[0].0),
            });
        }
    }
}
