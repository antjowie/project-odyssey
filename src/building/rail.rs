use std::f32::consts::PI;

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

#[derive(Bundle, Default)]
pub struct RailBundle {
    pub pbr: PbrBundle,
    pub building: Building,
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

// Start joint is the initial joint that is placed. You can als think of it as head and tail
//
const RAIL_START_JOINT: usize = 0;
const RAIL_END_JOINT: usize = 1;
const RAIL_JOINTS_MAX: usize = 2;

const RAIL_MIN_RADIANS: f32 = 10.0 * PI / 180.0;
const RAIL_MAX_RADIANS: f32 = 22.5 * PI / 180.0;
const RAIL_CURVES_MAX: usize = (RAIL_MAX_RADIANS / RAIL_MIN_RADIANS) as usize + 1;

/// Contains the details to build and connect a rail
#[derive(Component)]
pub struct RailPathState {
    pub joints: [RailPathJoint; RAIL_JOINTS_MAX],
}

impl RailPathState {
    fn new(
        self_entity: Entity,
        q: &mut Query<&mut RailPathState>,
        plan: &RailPlanner,
    ) -> RailPathState {
        let start = plan.start;
        let end = plan.end;
        let dir = (end - start).normalize();
        let size = ((end - start).length()).min(2.5);

        let mut state = RailPathState {
            joints: [
                RailPathJoint {
                    pos: start,
                    collision: Aabb3d::new(start + dir * size * 0.5, Vec3::splat(size)),
                    curves: [None; 3],
                },
                RailPathJoint {
                    pos: end,
                    collision: Aabb3d::new(end - dir * size * 0.5, Vec3::splat(size)),
                    curves: [None; 3],
                },
            ],
        };

        if let Some(target_joint_ref) = plan.target_joint {
            let mut target_state = q.get_mut(target_joint_ref.rail_entity).unwrap();
            let self_joint_idx = if target_state.joints[target_joint_ref.joint_idx].pos
                == state.joints[RAIL_START_JOINT].pos
            {
                RAIL_START_JOINT
            } else {
                RAIL_END_JOINT
            };

            let self_joint_ref = RailPathJointRef {
                rail_entity: self_entity,
                joint_idx: self_joint_idx,
            };

            connect_rail_joints(
                &mut state,
                self_joint_ref,
                &mut target_state,
                target_joint_ref,
            );
        }

        state
    }
}

pub struct RailPathJoint {
    pub pos: Vec3,
    pub collision: Aabb3d,
    pub curves: [Option<RailPathJointRef>; 3],
}

impl RailPathJoint {
    fn get_empty_curve_idx(&self) -> Option<usize> {
        info!("{:?}", self.curves);

        self.curves
            .iter()
            .enumerate()
            .find(|(_, c)| c.is_none())
            .and_then(|(i, _)| Some(i))
    }
}

fn connect_rail_joints(
    left_state: &mut RailPathState,
    left_joint_ref: RailPathJointRef,
    right_state: &mut RailPathState,
    right_joint_ref: RailPathJointRef,
) {
    // No spot left in the joint curves
    let left_joint = &mut left_state.joints[left_joint_ref.joint_idx];
    left_joint.curves[left_joint.get_empty_curve_idx().unwrap()] = Some(right_joint_ref);

    let right_joint = &mut right_state.joints[right_joint_ref.joint_idx];
    right_joint.curves[right_joint.get_empty_curve_idx().unwrap()] = Some(left_joint_ref);
}

// We store target_joint info in a specific struct since we can't reference other RailPathJoint
#[derive(Copy, Clone, Debug)]
pub struct RailPathJointRef {
    pub rail_entity: Entity,
    pub joint_idx: usize,
}

fn get_joint_collision(
    rail_path: &RailPathState,
    sphere: BoundingSphere,
) -> Option<&RailPathJoint> {
    if rail_path.joints[RAIL_START_JOINT]
        .collision
        .intersects(&sphere)
    {
        Some(&rail_path.joints[RAIL_START_JOINT])
    } else if rail_path.joints[RAIL_END_JOINT]
        .collision
        .intersects(&sphere)
    {
        Some(&rail_path.joints[RAIL_END_JOINT])
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
        gizmos.line(
            state.joints[RAIL_START_JOINT].pos,
            state.joints[RAIL_END_JOINT].pos,
            Color::WHITE,
        );
        let collision = get_joint_collision(state, cursor_sphere);

        let mut draw_joints = |joint: &RailPathJoint| {
            gizmos.cuboid(
                Transform::from_translation(joint.collision.center().into())
                    .with_scale(joint.collision.half_size().into()),
                if collision.is_some_and(|x| x.pos == joint.pos) {
                    Color::srgb(1.0, 0.0, 0.0)
                } else {
                    Color::WHITE
                },
            );
        };
        draw_joints(&state.joints[RAIL_START_JOINT]);
        draw_joints(&state.joints[RAIL_END_JOINT]);

        let mut draw_curves = |joint: &RailPathJoint| {
            for curve in joint.curves {
                if let Some(curve) = curve {
                    let target_joint = q.get(curve.rail_entity).unwrap();
                    gizmos.line(
                        joint.collision.center().into(),
                        target_joint.joints[curve.joint_idx]
                            .collision
                            .center()
                            .into(),
                        Color::srgb(0.1, 1.0, 0.1),
                    );
                }
            }
        };
        draw_curves(&state.joints[RAIL_START_JOINT]);
        draw_curves(&state.joints[RAIL_END_JOINT]);
    });
}
