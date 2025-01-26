//! Destroyer is a special type of placeable that destroys whatever it selected

use super::*;

pub(super) fn destroyer_plugin(app: &mut App) {
    app.add_systems(Startup, load_destroyer_asset);
    app.add_event::<DestroyEvent>();
    app.add_systems(
        Update,
        handle_destroyer.run_if(
            in_player_state(PlayerState::Building).and(is_placeable_preview(Placeable::Destroyer)),
        ),
    );
}

#[derive(Resource)]
pub struct DestroyerAsset {
    pub material: Handle<StandardMaterial>,
}

#[derive(Event)]
pub struct DestroyEvent;

/// Holds a handle to the original material
#[derive(Component)]
struct ConsiderForDestruction(Handle<StandardMaterial>);

fn load_destroyer_asset(mut c: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    c.insert_resource(DestroyerAsset {
        material: materials.add(Color::srgba_u8(255, 100, 100, 100)),
    });
}

fn handle_destroyer(
    mut c: Commands,
    mut q: ParamSet<(
        // Query for all placeable entities
        Query<
            (Entity, &mut MeshMaterial3d<StandardMaterial>),
            (With<Placeable>, Without<PlaceablePreview>),
        >,
        // Query for all entities considered for destructions
        Query<(
            Entity,
            &ConsiderForDestruction,
            &mut MeshMaterial3d<StandardMaterial>,
        )>,
    )>,
    player: Single<(&PlayerCursor, &ActionState<PlayerBuildAction>)>,
    mut ray_cast: MeshRayCast,
    ray_cast_filter: Query<(), (With<Placeable>, Without<PlaceablePreview>)>,
    asset: Res<DestroyerAsset>,
) {
    let (cursor, input) = player.into_inner();

    let hits = ray_cast.cast_ray(
        cursor.ray,
        &RayCastSettings::default()
            .always_early_exit()
            .with_filter(&|e| ray_cast_filter.contains(e)),
    );

    if hits.len() > 0 {
        if q.p1().contains(hits[0].0) == false {
            if let Ok((e, mut mat)) = q.p0().get_mut(hits[0].0) {
                let destroyer = ConsiderForDestruction(mat.0.clone());
                mat.0 = asset.material.clone();
                c.entity(e).insert(destroyer);
            }
        }
    }

    if input.just_pressed(&PlayerBuildAction::Interact) {
        c.trigger_targets(
            DestroyEvent,
            q.p1().iter().map(|(e, _, _)| e).collect::<Vec<Entity>>(),
        );
    }

    for (e, orig, mut mat) in q.p1().iter_mut() {
        if hits.len() > 0 && hits[0].0 == e {
            continue;
        }
        mat.0 = orig.0.clone();
        c.entity(e).remove::<ConsiderForDestruction>();
    }
}
