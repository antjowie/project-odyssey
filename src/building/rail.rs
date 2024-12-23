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
pub struct Rail {
    pub joints: [RailPathJoint; RAIL_JOINTS_MAX],
}

impl Rail {
    fn new(self_entity: Entity, q: &mut Query<&mut Rail>, plan: &RailPlanner) -> Rail {
        let start = plan.start;
        let end = plan.end;
        let dir = (end - start).normalize();
        let size = ((end - start).length()).min(2.5);

        let mut self_state = Rail {
            joints: [
                RailPathJoint {
                    pos: start,
                    forward: plan.start_forward,
                    collision: BoundingSphere::new(start + dir * size, size),
                    n_joints: [None; 3],
                },
                RailPathJoint {
                    pos: end,
                    forward: plan.end_forward,
                    collision: BoundingSphere::new(end - dir * size, size),
                    n_joints: [None; 3],
                },
            ],
        };

        let mut connect_joints = |other_joint_ref: RailPathJointRef| {
            let mut other_state = q.get_mut(other_joint_ref.rail_entity).unwrap();
            let self_joint_idx = if other_state.joints[other_joint_ref.joint_idx].pos
                == self_state.joints[RAIL_START_JOINT].pos
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
                &mut self_state,
                self_joint_ref,
                &mut other_state,
                other_joint_ref,
            );
        };

        // Check if we expanded from a joint
        if let Some(start_joint_ref) = plan.start_joint {
            connect_joints(start_joint_ref);
        }

        // Check if we clicked on a joint for end pos
        if let Some(end_joint_ref) = plan.end_joint {
            connect_joints(end_joint_ref);
        }

        self_state
    }
}

pub struct RailPathJoint {
    pub pos: Vec3,
    // Vector that represents the direction this joint goes towards. Used when we extend from this joint
    pub forward: Vec3,
    pub collision: BoundingSphere,
    // Neighbor joints
    pub n_joints: [Option<RailPathJointRef>; 3],
}

impl RailPathJoint {
    fn get_empty_curve_idx(&self) -> Option<usize> {
        info!("{:?}", self.n_joints);

        self.n_joints
            .iter()
            .enumerate()
            .find(|(_, c)| c.is_none())
            .and_then(|(i, _)| Some(i))
    }
}

fn connect_rail_joints(
    left_state: &mut Rail,
    left_joint_ref: RailPathJointRef,
    right_state: &mut Rail,
    right_joint_ref: RailPathJointRef,
) {
    // No spot left in the joint curves
    let left_joint = &mut left_state.joints[left_joint_ref.joint_idx];
    left_joint.n_joints[left_joint.get_empty_curve_idx().unwrap()] = Some(right_joint_ref);

    let right_joint = &mut right_state.joints[right_joint_ref.joint_idx];
    right_joint.n_joints[right_joint.get_empty_curve_idx().unwrap()] = Some(left_joint_ref);
}

// We store target_joint info in a specific struct since we can't reference other RailPathJoint
#[derive(Copy, Clone, Debug)]
pub struct RailPathJointRef {
    pub rail_entity: Entity,
    pub joint_idx: usize,
}

fn get_joint_collision(rail_path: &Rail, sphere: BoundingSphere) -> Option<&RailPathJoint> {
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
    q: Query<&Rail>,
    preview: Query<&RailPlanner>,
    cursor: Query<&PlayerCursor, With<NetOwner>>,
) {
    let cursor = cursor.single();
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);
    let preview_exists = !preview.is_empty();

    q.into_iter().for_each(|state| {
        // Draw line
        let length = state.joints[RAIL_START_JOINT]
            .pos
            .distance(state.joints[RAIL_END_JOINT].pos);

        let points = [[
            state.joints[RAIL_START_JOINT].pos,
            state.joints[RAIL_START_JOINT].pos
                + (-state.joints[RAIL_START_JOINT].forward) * length * 0.5,
            state.joints[RAIL_END_JOINT].pos
                + (-state.joints[RAIL_END_JOINT].forward) * length * 0.5,
            state.joints[RAIL_END_JOINT].pos,
        ]];

        let curve = CubicBezier::new(points).to_curve().unwrap();
        const STEPS: usize = 10;
        gizmos.linestrip(curve.iter_positions(STEPS), Color::WHITE);

        // Draw forwards
        gizmos.line(
            state.joints[RAIL_START_JOINT].pos,
            state.joints[RAIL_START_JOINT].pos + state.joints[RAIL_START_JOINT].forward,
            Color::srgb(1.0, 0.1, 0.1),
        );
        gizmos.line(
            state.joints[RAIL_START_JOINT].pos,
            state.joints[RAIL_START_JOINT].pos + state.joints[RAIL_START_JOINT].forward,
            Color::srgb(0.1, 0.1, 1.0),
        );

        let collision = get_joint_collision(state, cursor_sphere);

        let mut draw_joint = |joint: &RailPathJoint| {
            // Draw collision
            let collides_joint = collision.is_some_and(|x| x.pos == joint.pos);

            gizmos.sphere(
                Isometry3d::from_translation(joint.collision.center),
                joint.collision.radius(),
                if collides_joint {
                    Color::srgb(1.0, 0.0, 0.0)
                } else {
                    Color::WHITE
                },
            );

            if collides_joint && !preview_exists {
                let center: Vec3 = joint.collision.center.into();
                gizmos
                    .arrow(
                        center,
                        center + joint.forward * joint.collision.radius() * 2.,
                        Color::srgb(0., 1., 0.),
                    )
                    .with_tip_length(joint.collision.radius());
            }

            // Draw neighbors
            for neighbor in joint.n_joints {
                if let Some(neighbor) = neighbor {
                    // If update_rail_planner runs parallel to this system, the entity is already created but
                    // the component is not yet created. We could also chain this system after update_rail_planner
                    // but I think it's faster to guard against none values
                    if let Ok(target_joint) = q.get(neighbor.rail_entity) {
                        gizmos.line(
                            joint.collision.center().into(),
                            target_joint.joints[neighbor.joint_idx]
                                .collision
                                .center()
                                .into(),
                            Color::srgb(0.1, 1.0, 0.1),
                        );
                    }
                }
            }
        };
        draw_joint(&state.joints[RAIL_START_JOINT]);
        draw_joint(&state.joints[RAIL_END_JOINT]);
    });
}
