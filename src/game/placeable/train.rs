use avian3d::prelude::*;
use rail::Rail;

use super::*;
use crate::spline::Spline;

pub(super) fn train_plugin(app: &mut App) {
    app.add_systems(Startup, load_train_asset);
    app.add_systems(
        Update,
        handle_train_placement.run_if(
            in_player_state(PlayerState::Building).and(is_placeable_preview(Placeable::Train)),
        ),
    );
}

#[derive(Component)]
#[require(Placeable(||Placeable::Train), Name(|| Name::new("Train")))]
pub struct Train;

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

fn handle_train_placement(
    mut c: Commands,
    mut q: Query<(&mut PlayerCursor, &ActionState<PlayerBuildAction>)>,
    mut preview: Query<(&mut Transform, &mut PlaceablePreview), With<Train>>,
    cameras: Query<(&Camera, &Transform), Without<Train>>,
    rails: Query<&Spline, With<Rail>>,
    // mut gizmos: Gizmos,
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
    let (_camera, transform) = cameras
        .iter()
        .find(|(camera, _transform)| camera.is_active)
        .unwrap();

    let ray = Ray3d {
        origin: transform.translation,
        direction: Dir3::new(cursor.build_pos - transform.translation).unwrap(),
    };
    let hits = ray_cast.cast_ray(
        ray,
        &RayCastSettings::default().with_filter(&|e| rails.contains(e)),
    );
    let mut pos = cursor.build_pos;
    let mut forward = preview.0.forward();
    if hits.len() > 0 {
        if let Ok(spline) = rails.get(hits[0].0) {
            let mut points = spline.create_curve_points(spline.create_curve_control_points());
            points.push(spline.controls[1].pos - spline.controls[1].forward);

            let (start, end) = points
                .iter()
                .zip(points.iter().skip(1))
                .min_by(|x, y| {
                    let left = pos.distance_squared(*x.0) + pos.distance_squared(*x.1);
                    let right = pos.distance_squared(*y.0) + pos.distance_squared(*y.1);
                    left.total_cmp(&right)
                })
                .unwrap();

            // Calculate perpendicular vec from pos to rail
            forward = Dir3::new(end - start).unwrap();
            let right = forward.cross(Vec3::Y);

            let offset = (start - pos).project_onto(right);
            pos += offset;
            cursor.manual_rotation = 0.0;

            if !*previous_had_hit {
                *align_to_right = forward.dot(preview.0.forward().as_vec3()) > 0.;
            }

            if input.just_pressed(&PlayerBuildAction::Rotate) {
                *align_to_right = !*align_to_right;
            }

            if !*align_to_right {
                forward = Dir3::new(forward.as_vec3() * -1.0).unwrap();
            }

            *previous_had_hit = true;
        }
    } else {
        forward = Quat::from_rotation_y(cursor.manual_rotation) * forward;

        *previous_had_hit = false;
    }

    cursor.manual_rotation = 0.0;
    preview.0.translation = pos;
    preview.0.look_at(pos + forward.as_vec3(), Vec3::Y);

    // Overlap check
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
            Train,
            preview.0.clone(),
            Mesh3d(train.mesh.clone()),
            MeshMaterial3d(train.material.clone()),
            train.collider.clone(),
        ));
    }
}
