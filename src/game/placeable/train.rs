use rail::Rail;

use crate::spline::Spline;

use super::*;

use super::{in_player_state, PlayerBuildAction};

pub(super) fn train_plugin(app: &mut App) {
    app.add_systems(Startup, load_train_asset);
    app.add_systems(
        Update,
        preview_train_placement
            .run_if(in_player_state(PlayerState::Building).and(is_placeable(Placeable::Train))),
    );
}

#[derive(Resource)]
pub struct TrainAsset {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

fn load_train_asset(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    c.insert_resource(TrainAsset {
        mesh: meshes.add(Extrusion::new(
            Triangle2d::new(Vec2::new(-0.5, 1.), Vec2::new(0.5, 1.), Vec2::new(0., -1.)),
            1.,
        )),
        material: materials.add(Color::BLACK),
    });
}

fn preview_train_placement(
    q: Query<(&PlayerCursor, &ActionState<PlayerBuildAction>)>,
    cameras: Query<(&Camera, &Transform)>,
    rails: Query<&Spline, With<Rail>>,
    mut gizmos: Gizmos,
    mut ray_cast: MeshRayCast,
) {
    let (cursor, input) = q.single();
    let (_camera, transform) = cameras
        .iter()
        .find(|(camera, _transform)| camera.is_active)
        .unwrap();

    let ray = Ray3d {
        origin: transform.translation,
        direction: Dir3::new(cursor.build_pos - transform.translation).unwrap(),
    };
    let hits = ray_cast.cast_ray(ray, &RayCastSettings::default());
    let mut pos = cursor.build_pos;
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
            let forward = (end - start).normalize();
            let right = forward.cross(Vec3::Y);

            let offset = (start - pos).project_onto(right);
            pos += offset;
        }
    }
    gizmos.sphere(Isometry3d::from_translation(pos), 10., Color::WHITE);
}
