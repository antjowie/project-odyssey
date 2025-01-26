//! Any placeable are things that can be placed
use super::*;
use bevy::{ecs::traversal::Traversal, pbr::NotShadowCaster};

use destroyer::*;
use rail::*;
use train::*;
pub mod destroyer;
pub mod rail;
pub mod train;

pub(super) fn placeable_plugin(app: &mut App) {
    app.add_systems(Startup, load_assets);
    app.add_plugins(destroyer_plugin);
    app.add_plugins(rail_plugin);
    app.add_plugins(train_plugin);
    app.add_event::<PlaceablePreviewChangedEvent>();

    app.add_systems(
        Update,
        (
            cleanup_build_preview_on_state_change.run_if(on_event::<PlayerStateEvent>),
            update_picked_placeable.run_if(in_player_state(PlayerState::Building)),
            create_or_update_placeable_preview
                .run_if(on_event::<PlaceablePreviewChangedEvent>.or(on_event::<PlayerStateEvent>)),
            (
                on_add_build_preview_component,
                update_build_preview_material,
            )
                .chain(),
        ),
    );
    app.add_systems(PostUpdate, on_remove_build_preview_component);
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
    new: Placeable,
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
    orig_material: MeshMaterial3d<StandardMaterial>,
    pub valid: bool,
}

impl PlaceablePreview {
    pub fn new(state_instigator: Entity) -> PlaceablePreview {
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

pub fn is_placeable_preview(
    placeable: Placeable,
) -> impl FnMut(Query<&Placeable, With<PlayerState>>) -> bool {
    move |query: Query<&Placeable, With<PlayerState>>| {
        !query.is_empty() && *query.single() == placeable
    }
}

fn create_or_update_placeable_preview(
    mut c: Commands,
    mut ev: EventReader<PlaceablePreviewChangedEvent>,
    player_state: Single<(Entity, &Placeable), With<PlayerState>>,
    previews: Query<Entity, With<PlaceablePreview>>,
    train: Res<TrainAsset>,
) {
    let e_player = player_state.0;

    let spawn = |placeable: &Placeable, c: &mut Commands| match placeable {
        Placeable::Rail => {}
        Placeable::Train => {
            c.spawn((
                PlaceablePreview::new(e_player),
                Mesh3d(train.mesh.clone()),
                MeshMaterial3d(train.material.clone()),
            ));
        }
        Placeable::Destroyer => {}
    };

    if previews.is_empty() {
        spawn(&player_state.1, &mut c);
    } else {
        previews.iter().for_each(|e| c.entity(e).try_despawn());

        for e in ev.read() {
            spawn(&e.new, &mut c);
        }
    }
}

fn cleanup_build_preview_on_state_change(
    mut c: Commands,
    q: Query<Entity, With<PlaceablePreview>>,
    mut event: EventReader<PlayerStateEvent>,
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
        c.entity(e)
            .insert((NotShadowCaster, PickingBehavior::IGNORE));
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
            c.entity(entity).remove::<PickingBehavior>();
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

fn update_picked_placeable(
    mut q: Query<(&ActionState<PlayerBuildAction>, &mut Placeable)>,
    mut ev: EventWriter<PlaceablePreviewChangedEvent>,
) {
    q.iter_mut().for_each(|(input, mut placeable)| {
        let old_placeable = placeable.clone();
        if input.just_pressed(&PlayerBuildAction::PickRail) {
            *placeable = Placeable::Rail;
        }
        if input.just_pressed(&PlayerBuildAction::PickTrain) {
            *placeable = Placeable::Train;
        }
        if input.just_pressed(&PlayerBuildAction::PickDestroy) {
            *placeable = Placeable::Destroyer;
        }
        if *placeable.bypass_change_detection() != old_placeable {
            ev.send(PlaceablePreviewChangedEvent {
                new: placeable.clone(),
            });
        }
    });
}
