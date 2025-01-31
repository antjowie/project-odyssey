use super::*;
use crate::spline::Spline;
use avian3d::prelude::*;
use bevy_rand::{global::GlobalEntropy, prelude::*};
use rail::Rail;
use rand_core::RngCore;

pub(super) fn train_plugin(app: &mut App) {
    app.add_systems(Startup, load_train_asset);
    app.add_systems(
        Update,
        (
            handle_train_placement.run_if(
                in_player_state(PlayerState::Building).and(is_placeable_preview(Placeable::Train)),
            ),
            move_trains,
        ),
    );
}

#[derive(Component)]
#[require(Placeable(||Placeable::Train), Name(|| Name::new("Train")))]
pub struct Train {
    /// The alpha on the current rail the train is traversing
    pub t: f32,
    pub rail: Entity,
}

impl Train {
    pub fn traverse(
        &self,
        distance: f32,
        t: f32,
        forward: Dir3,
        rail_id: Entity,
        rail: &Rail,
        spline: &Spline,
        rails: &Query<(&Rail, &Spline)>,
        intersections: &RailIntersections,
        rng: &mut GlobalEntropy<WyRand>,
    ) -> TrainTraverseResult {
        match rail.traverse(t, &forward, distance, spline) {
            TraverseResult::Intersection {
                t,
                pos,
                forward,
                remaining_distance,
                intersection_id,
            } => {
                let intersection = intersections.intersections.get(&intersection_id).unwrap();
                let options = intersection.get_curve_options(&forward);
                if options.is_empty() {
                    // For now we'll just flip the train
                    self.traverse(
                        remaining_distance,
                        t,
                        -forward,
                        rail_id,
                        rail,
                        spline,
                        rails,
                        intersections,
                        rng,
                    )
                } else {
                    // For now pick random rail option
                    let new_rail_id = options[rng.next_u32() as usize % options.len()];
                    let (new_rail, new_spline) = rails.get(new_rail_id).unwrap();
                    let t = new_spline.t_from_pos(&pos).round();
                    self.traverse(
                        remaining_distance,
                        t,
                        forward,
                        new_rail_id,
                        new_rail,
                        new_spline,
                        rails,
                        intersections,
                        rng,
                    )
                }
            }
            TraverseResult::End { t, pos, forward } => TrainTraverseResult {
                t,
                pos,
                forward,
                rail: rail_id,
            },
        }
    }
}

pub struct TrainTraverseResult {
    pub t: f32,
    pub pos: Vec3,
    pub forward: Dir3,
    pub rail: Entity,
}

#[derive(Resource)]
pub struct TrainAsset {
    pub mesh: Handle<Mesh>,
    pub collider: Collider,
    pub material: Handle<StandardMaterial>,
}

fn load_train_asset(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Tetrahedron::new(
        vec3(0.0, 2.0, 2.0),
        vec3(-2.0, 0.0, 2.0),
        vec3(2.0, 0.0, 2.0),
        vec3(0.0, 0.0, -2.0),
    ));
    c.insert_resource(TrainAsset {
        collider: Collider::convex_hull_from_mesh(&meshes.get(&mesh).unwrap()).unwrap(),
        mesh,
        // https://www.designpieces.com/palette/ns-color-palette-hex-and-rgb/
        material: materials.add(Color::srgb_u8(255, 198, 30)),
    });
}

fn move_trains(
    mut q: Query<(&mut Transform, &mut Train)>,
    rails: Query<(&Rail, &Spline)>,
    time: Res<Time>,
    intersections: Res<RailIntersections>,
    mut rng: GlobalEntropy<WyRand>,
    // mut gizmos: Gizmos,
) {
    q.iter_mut().for_each(|(mut t, mut train)| {
        let (rail, spline) = rails.get(train.rail).unwrap();
        let distance = 10.0 * time.delta_secs();
        let result = train.traverse(
            distance,
            train.t,
            t.forward(),
            train.rail,
            rail,
            spline,
            &rails,
            &intersections,
            &mut rng,
        );

        // let delta = result.pos - t.translation;
        // for i in 0..200 {
        //     let i = i as f32;
        //     let pos = t.translation + delta * i;
        //     gizmos.sphere(Isometry3d::from_translation(pos.clone()), 0.3, GREEN_500);
        // }

        train.rail = result.rail;
        train.t = result.t;
        t.translation = result.pos;
        let up = t.up();
        t.look_at(result.pos + result.forward.as_vec3(), up);
    });
}

fn handle_train_placement(
    mut c: Commands,
    mut q: Query<(&mut PlayerCursor, &ActionState<PlayerBuildAction>)>,
    mut preview: Query<(&mut Transform, &mut PlaceablePreview)>,
    rails: Query<&Spline, With<Rail>>,
    mut gizmos: Gizmos,
    mut ray_cast: MeshRayCast,
    mut previous_had_hit: Local<bool>,
    mut align_to_right: Local<bool>,
    train: Res<TrainAsset>,
    spatial_query: SpatialQuery,
) {
    if preview.is_empty() {
        return;
    }
    let mut preview = preview.single_mut();

    let (mut cursor, input) = q.single_mut();
    let hits = ray_cast.cast_ray(
        cursor.ray,
        &RayCastSettings::default().with_filter(&|e| rails.contains(e)),
    );

    let mut pos = cursor.build_pos;
    let mut forward = preview.0.forward();
    let mut target_rail = None;
    let mut target_spline = None;

    if hits.len() > 0 {
        if let Ok(spline) = rails.get(hits[0].0) {
            let t = spline.t_from_pos(&pos);
            pos = spline.position(t);
            forward = spline.forward(t);
            cursor.manual_rotation = 0.0;
            gizmos.line(cursor.build_pos + Vec3::Y, pos + Vec3::Y, RED_500);
            for point in spline.curve_points() {
                gizmos.sphere(Isometry3d::from_translation(*point), 0.2, RED_500);
            }

            if !*previous_had_hit {
                *align_to_right = forward.dot(preview.0.forward().as_vec3()) > 0.;
            }

            if input.just_pressed(&PlayerBuildAction::Rotate) {
                *align_to_right = !*align_to_right;
            }

            if !*align_to_right {
                forward = Dir3::new(forward.as_vec3() * -1.0).unwrap();
            }

            target_rail = Some(hits[0].0);
            target_spline = Some(spline);
            *previous_had_hit = true;
        }
    } else {
        forward = Quat::from_rotation_y(cursor.manual_rotation) * forward;

        *previous_had_hit = false;
    }

    cursor.manual_rotation = 0.0;
    preview.0.translation = pos;
    preview.0.look_at(pos + forward.as_vec3(), Vec3::Y);

    // Overlap check for other trains
    let can_place = *previous_had_hit
        && spatial_query
            .shape_intersections(
                &train.collider,
                preview.0.translation,
                preview.0.rotation,
                &SpatialQueryFilter::default(),
            )
            .len()
            == 0;

    preview.1.valid = can_place;

    if can_place && input.just_pressed(&PlayerBuildAction::Interact) {
        c.spawn((
            Train {
                t: target_spline.unwrap().t_from_pos(&pos),
                rail: target_rail.unwrap(),
            },
            preview.0.clone(),
            Mesh3d(train.mesh.clone()),
            MeshMaterial3d(train.material.clone()),
            train.collider.clone(),
        ))
        .observe(on_destroy_default);
    }
}
