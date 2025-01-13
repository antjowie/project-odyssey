use super::*;
use bevy::{
    math::bounding::{BoundingSphere, IntersectsVolume},
    utils::HashMap,
};

use rail_planner::*;

pub mod rail_graph;
pub mod rail_planner;

pub(super) fn rail_plugin(app: &mut App) {
    // app.add_systems(Update, (on_place_rail, debug_draw_rail_path));
    app.add_plugins((
        rail_graph::rail_graph_plugin,
        rail_planner::rail_planner_plugin,
    ));
    app.add_systems(Update, debug_draw_rail_path);
    app.init_resource::<RailIntersections>();
}

// Start joint is the initial joint that is placed. You can als think of it as head and tail
const RAIL_START_JOINT: usize = 0;
const RAIL_END_JOINT: usize = 1;
const RAIL_JOINTS_MAX: usize = 2;

const RAIL_MIN_LENGTH: f32 = 10.;
const RAIL_MIN_RADIANS: f32 = 10.0 * PI / 180.0;
const RAIL_MAX_RADIANS: f32 = 22.5 * PI / 180.0;
const RAIL_CURVES_MAX: usize = (RAIL_MAX_RADIANS / RAIL_MIN_RADIANS) as usize + 1;

/// Contains the details to build and connect a rail
#[derive(Component)]
pub struct Rail {
    pub joints: [RailJoint; RAIL_JOINTS_MAX],
}

impl Rail {
    fn new(
        self_entity: Entity,
        intersections: &mut ResMut<RailIntersections>,
        plan: &RailPlanner,
    ) -> Rail {
        let start = plan.start;
        let end = plan.end;
        let dir = (end - start).normalize();
        let size = 2.5;

        // Create the intersections
        let mut create_new_intersection = |right_forward: Vec3| -> u32 {
            let idx = intersections.get_available_index();
            intersections.intersections.insert(
                idx,
                RailIntersection {
                    right_forward,
                    ..default()
                },
            );
            idx
        };

        let start_intersection_id = plan
            .start_intersection_id
            .unwrap_or_else(|| create_new_intersection(dir));
        let end_intersection_id = plan
            .end_intersection_id
            .unwrap_or_else(|| create_new_intersection(dir));

        let self_state = Rail {
            joints: [
                RailJoint {
                    pos: start,
                    forward: plan.start_forward,
                    collision: BoundingSphere::new(start + dir * size, size),
                    intersection_id: start_intersection_id,
                },
                RailJoint {
                    pos: end,
                    forward: plan.end_forward,
                    collision: BoundingSphere::new(end - dir * size, size),
                    intersection_id: end_intersection_id,
                },
            ],
        };

        let connect_intersection =
            |entity: Entity, forward: &Vec3, intersection: &mut RailIntersection| {
                let side = if intersection.right_forward.dot(*forward) > 0. {
                    &mut intersection.right
                } else {
                    &mut intersection.left
                };

                side[RailIntersection::get_empty_idx(side).unwrap()] = Some(entity);
            };

        let mut start_intersection = intersections
            .intersections
            .get_mut(&start_intersection_id)
            .unwrap();

        connect_intersection(self_entity, &-plan.start_forward, &mut start_intersection);

        let mut end_intersection = intersections
            .intersections
            .get_mut(&end_intersection_id)
            .unwrap();
        connect_intersection(self_entity, &-plan.end_forward, &mut end_intersection);

        self_state
    }
}

/// RailJoint represents the end points of a rail, used to construct a curve
pub struct RailJoint {
    pub pos: Vec3,
    /// Vector that represents the direction this joint goes towards (if we were to expand)
    pub forward: Vec3,
    pub collision: BoundingSphere,
    pub intersection_id: u32,
}

#[derive(Resource, Default)]
pub struct RailIntersections {
    pub intersections: HashMap<u32, RailIntersection>,
    next_index: u32,
    available_indexes: Vec<u32>,
}

impl RailIntersections {
    fn get_available_index(&mut self) -> u32 {
        if self.available_indexes.len() > 0 {
            return self.available_indexes.pop().unwrap();
        }
        self.next_index += 1;
        return self.next_index;
    }
}

#[derive(Default, Debug)]
pub struct RailIntersection {
    pub left: [Option<Entity>; RAIL_CURVES_MAX],
    pub right: [Option<Entity>; RAIL_CURVES_MAX],
    /// The right direction, required to know where to store a Rail
    pub right_forward: Vec3,
}

impl RailIntersection {
    fn get_empty_idx(intersections: &[Option<Entity>; RAIL_CURVES_MAX]) -> Option<usize> {
        intersections
            .iter()
            .enumerate()
            .find(|(_, v)| v.is_none())
            .and_then(|(i, _)| Some(i))
    }
}

pub fn create_curve_control_points(
    start: Vec3,
    start_forward: Vec3,
    end: Vec3,
    end_forward: Vec3,
) -> [[Vec3; 4]; 1] {
    let length = (end - start).length();

    [[
        start,
        start - start_forward * length * 0.5,
        end - end_forward * length * 0.5,
        end,
    ]]
}

/// Use points generated by create_curve_points
pub fn create_curve_points(points: [[Vec3; 4]; 1]) -> Vec<Vec3> {
    let start = points[0][0];
    let end = points[0][3];
    let segments = ((start.distance(end) / RAIL_MIN_LENGTH).round() as usize)
        .max(2)
        .min(10);
    CubicBezier::new(points)
        .to_curve()
        .unwrap()
        .iter_positions(segments)
        .collect()
}

fn get_joint_collision(rail_path: &Rail, sphere: BoundingSphere) -> Option<&RailJoint> {
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
    cursor: Query<&PlayerCursor>,
) {
    let cursor = cursor.single();
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);
    let preview_exists = !preview.is_empty();

    q.into_iter().for_each(|state| {
        // Draw line
        let points = create_curve_control_points(
            state.joints[RAIL_START_JOINT].pos,
            state.joints[RAIL_START_JOINT].forward,
            state.joints[RAIL_END_JOINT].pos,
            state.joints[RAIL_END_JOINT].forward,
        );

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

        let mut draw_joint = |joint: &RailJoint| {
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
            // for neighbor in joint.n_joints {
            //     if let Some(neighbor) = neighbor {
            //         // If update_rail_planner runs parallel to this system, the entity is already created but
            //         // the component is not yet created. We could also chain this system after update_rail_planner
            //         // but I think it's faster to guard against none values
            //         if let Ok(target_joint) = q.get(neighbor.rail_entity) {
            //             gizmos.line(
            //                 joint.collision.center().into(),
            //                 target_joint.joints[neighbor.joint_idx]
            //                     .collision
            //                     .center()
            //                     .into(),
            //                 Color::srgb(0.1, 1.0, 0.1),
            //             );
            //         }
            //     }
            // }
        };
        draw_joint(&state.joints[RAIL_START_JOINT]);
        draw_joint(&state.joints[RAIL_END_JOINT]);
    });
}
