//! Destroyer is a special type of placeable that destroys whatever it selected

use super::*;

pub(super) fn destroyer_plugin(app: &mut App) {
    app.add_systems(Startup, load_destroyer_asset);
    app.add_event::<DestroyEvent>();
    app.add_systems(Update, handle_destroyer.in_set(GameSet::Despawn));
}

#[derive(Resource)]
pub struct DestroyerAsset {
    pub material: Handle<StandardMaterial>,
}

#[derive(Event)]
pub struct DestroyEvent;

/// Holds a handle to the original material
#[derive(Component)]
struct ConsiderForDestruction(Option<Handle<StandardMaterial>>);

fn load_destroyer_asset(mut c: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    c.insert_resource(DestroyerAsset {
        material: materials.add(Color::srgba_u8(255, 100, 100, 100)),
    });
}

pub fn on_destroy_default(trigger: Trigger<DestroyEvent>, mut c: Commands) {
    let e = trigger.entity();
    c.entity(e).despawn_recursive();
}

fn handle_destroyer(
    mut c: Commands,
    mut to_destroy: Query<(Entity, &ConsiderForDestruction)>,
    destroyables: Query<Entity, (With<Placeable>, Without<PlaceablePreview>)>,
    player: Query<(
        &PlayerCursor,
        &ActionState<PlayerBuildAction>,
        &PlayerState,
        &Placeable,
    )>,
    mut ray_cast: MeshRayCast,
    asset: Res<DestroyerAsset>,
    mut materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
    children: Query<&Children>,
    parents: Query<&Parent>,
) {
    if player.is_empty() {
        return;
    }

    let (cursor, input, state, placeable) = player.single();

    let mut hovered = None;
    if state == &PlayerState::Building && placeable == &Placeable::Destroyer {
        let hits = ray_cast.cast_ray(
            cursor.ray,
            &RayCastSettings::default()
                .with_visibility(RayCastVisibility::Any)
                .always_early_exit(),
        );
        if hits.len() > 0 {
            if destroyables.contains(hits[0].0) {
                hovered = Some(hits[0].0);
            } else {
                hovered = parents
                    .iter_ancestors(hits[0].0)
                    .find(|x| destroyables.contains(*x));
            }
        }

        if let Some(hovered) = hovered {
            // If the hovered entity is not yet updated
            if to_destroy.contains(hovered) == false {
                let mut handle = |e| {
                    let handle = materials.get_mut(e).ok();
                    c.entity(e).insert(if let Some(mut handle) = handle {
                        let destroyer = ConsiderForDestruction(Some(handle.0.clone()));
                        handle.0 = asset.material.clone();
                        destroyer
                    } else {
                        ConsiderForDestruction(None)
                    });
                };

                handle(hovered);
                children.iter_descendants(hovered).for_each(handle);
            }
        }

        if input.just_pressed(&PlayerBuildAction::Interact) {
            c.trigger_targets(
                DestroyEvent,
                to_destroy.iter().map(|(e, _)| e).collect::<Vec<Entity>>(),
            );
        }
    }

    // For all unhovered entities, undo their destruction
    for (destroy, orig_mat) in to_destroy.iter_mut() {
        if let Some(hovered) = hovered {
            if hovered == destroy || parents.iter_ancestors(destroy).any(|x| x == hovered) {
                continue;
            }
        }

        if let Some(orig_mat) = &orig_mat.0 {
            if let Ok(mut mat) = materials.get_mut(destroy) {
                mat.0 = orig_mat.clone();
            }
        }
        c.entity(destroy).remove::<ConsiderForDestruction>();
    }
}
