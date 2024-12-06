use crate::building::*;
use crate::game::*;
use rail_planner::*;

use bevy::math::bounding::{Aabb3d, BoundingSphere, BoundingVolume, IntersectsVolume};
use leafwing_input_manager::prelude::*;

mod rail_graph;
mod rail_planner;

pub fn rail_plugin(app: &mut App) {
    // app.add_systems(Update, (on_place_rail, debug_draw_rail_path));
    app.add_plugins((
        rail_graph::rail_graph_plugin,
        rail_planner::rail_planner_plugin,
    ));
    app.add_systems(Update, debug_draw_rail_path);
}

#[derive(Bundle)]
pub struct RailBundle {
    pub pbr: PbrBundle,
    pub building: Building,
    pub state: RailPathState,
}

impl RailBundle {
    fn new(state: RailPathState) -> RailBundle {
        RailBundle {
            pbr: PbrBundle::default(),
            building: Building::default(),
            state: state,
        }
    }
}

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
#[derive(Component)]
pub struct RailPathState {
    pub start_joint: RailPathJoint,
    pub end_joint: RailPathJoint,
}

impl RailPathState {
    fn new(plan: &RailPlanner) -> RailPathState {
        let start = plan.start;
        let end = plan.end;
        let size = ((end - start).length()).min(2.5);

        // TODO: Actually update joint bindings
        RailPathState {
            start_joint: RailPathJoint {
                pos: start,
                collision: Aabb3d::new(start, Vec3::splat(size)),
                left: None,
                straight: None,
                right: None,
            },
            end_joint: RailPathJoint {
                pos: end,
                collision: Aabb3d::new(end, Vec3::splat(size)),
                left: None,
                straight: None,
                right: None,
            },
        }
    }
}

pub struct RailPathJoint {
    pub pos: Vec3,
    pub collision: Aabb3d,
    pub left: Option<Entity>,
    pub straight: Option<Entity>,
    pub right: Option<Entity>,
}

impl Default for RailPathJoint {
    fn default() -> Self {
        RailPathJoint {
            collision: Aabb3d::new(Vec3::ZERO, Vec3::ZERO),
            ..default()
        }
    }
}

fn get_joint_collision(
    rail_path: &RailPathState,
    sphere: BoundingSphere,
) -> Option<&RailPathJoint> {
    if rail_path.start_joint.collision.intersects(&sphere) {
        Some(&rail_path.start_joint)
    } else if rail_path.end_joint.collision.intersects(&sphere) {
        Some(&rail_path.end_joint)
    } else {
        None
    }
}

pub fn debug_draw_rail_path(
    mut gizmos: Gizmos,
    q: Query<&RailPathState>,
    cursor: Query<&PlayerCursor, With<NetOwner>>,
) {
    let cursor = cursor.single();
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    q.into_iter().for_each(|state| {
        gizmos.line(state.start_joint.pos, state.end_joint.pos, Color::WHITE);
        let collision = get_joint_collision(state, cursor_sphere);
        gizmos.cuboid(
            Transform::from_translation(state.start_joint.collision.center().into())
                .with_scale(state.start_joint.collision.half_size().into()),
            if collision.is_some_and(|joint| joint.pos == state.start_joint.pos) {
                Color::srgb(1.0, 0.0, 0.0)
            } else {
                Color::WHITE
            },
        );
        gizmos.cuboid(
            Transform::from_translation(state.end_joint.collision.center().into())
                .with_scale(state.end_joint.collision.half_size().into()),
            if collision.is_some_and(|joint| joint.pos == state.end_joint.pos) {
                Color::srgb(1.0, 0.0, 0.0)
            } else {
                Color::WHITE
            },
        );
    });
}
