use super::*;
use bevy::{
    math::bounding::{BoundingSphere, IntersectsVolume},
    utils::HashMap,
};

use bevy_egui::{egui, EguiContexts};
use bounding::BoundingVolume;
use rail_planner::*;

pub mod rail_graph;
pub mod rail_planner;

pub(super) fn rail_plugin(app: &mut App) {
    // app.add_systems(Update, (on_place_rail, debug_draw_rail_path));
    app.add_plugins((
        rail_graph::rail_graph_plugin,
        rail_planner::rail_planner_plugin,
    ));
    app.add_systems(Update, (debug_rail_path, debug_rail_intersections));
    app.init_resource::<RailIntersections>();
}

// Start joint is the initial joint that is placed. You can als think of it as head and tail
const RAIL_START_JOINT: usize = 0;
const RAIL_END_JOINT: usize = 1;
const RAIL_JOINTS_MAX: usize = 2;

const RAIL_MIN_LENGTH: f32 = 10.;
const RAIL_MIN_DELTA_RADIANS: f32 = 15.0 * PI / 180.;
const RAIL_MAX_RADIANS: f32 = 22.5 * PI / 180.;
const RAIL_CURVES_MAX: usize = (PI / RAIL_MIN_DELTA_RADIANS) as usize;

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
        let size = 2.5;

        // Create the intersections
        let mut create_new_intersection = |pos: Vec3, right_forward: Vec3| -> u32 {
            let idx = intersections.get_available_index();
            intersections.intersections.insert(
                idx,
                RailIntersection {
                    right_forward,
                    left: [None; RAIL_CURVES_MAX],
                    right: [None; RAIL_CURVES_MAX],
                    collision: BoundingSphere::new(pos, size),
                },
            );
            idx
        };

        let start_intersection_id = plan
            .start_intersection_id
            .unwrap_or_else(|| create_new_intersection(start, plan.end_forward));
        let end_intersection_id = plan
            .end_intersection_id
            .unwrap_or_else(|| create_new_intersection(end, plan.end_forward));

        let self_state = Rail {
            joints: [
                RailJoint {
                    pos: start,
                    forward: plan.start_forward,
                    intersection_id: start_intersection_id,
                },
                RailJoint {
                    pos: end,
                    forward: plan.end_forward,
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

        connect_intersection(self_entity, &plan.start_forward, &mut start_intersection);

        let mut end_intersection = intersections
            .intersections
            .get_mut(&end_intersection_id)
            .unwrap();
        connect_intersection(self_entity, &plan.end_forward, &mut end_intersection);

        self_state
    }
}

/// RailJoint represents the end points of a rail, used to construct a curve
pub struct RailJoint {
    pub pos: Vec3,
    /// Vector that represents the direction this joint goes towards (if we were to expand)
    pub forward: Vec3,
    pub intersection_id: u32,
}

#[derive(Resource, Default)]
pub struct RailIntersections {
    pub intersections: HashMap<u32, RailIntersection>,
    next_index: u32,
    available_indexes: Vec<u32>,
}

impl RailIntersections {
    pub fn get_intersect_collision(
        &self,
        sphere: &BoundingSphere,
    ) -> Option<(&u32, &RailIntersection)> {
        self.intersections
            .iter()
            .find(|x| x.1.collision.intersects(sphere))
    }

    fn get_available_index(&mut self) -> u32 {
        if self.available_indexes.len() > 0 {
            return self.available_indexes.pop().unwrap();
        }
        self.next_index += 1;
        return self.next_index;
    }
}

/// Can be considered as a node in a graph
/// A junction is supported by inserting an intersection
/// Traffic control is controlled by inserting an intersection, to split traffic groups
#[derive(Debug)]
pub struct RailIntersection {
    pub left: [Option<Entity>; RAIL_CURVES_MAX],
    pub right: [Option<Entity>; RAIL_CURVES_MAX],
    /// The right direction, required to know where to store a Rail
    pub right_forward: Vec3,
    pub collision: BoundingSphere,
}

impl RailIntersection {
    pub fn get_empty_idx(intersections: &[Option<Entity>; RAIL_CURVES_MAX]) -> Option<usize> {
        intersections
            .iter()
            .enumerate()
            .find(|(_, v)| v.is_none())
            .and_then(|(i, _)| Some(i))
    }

    pub fn min_angle_relative_to_others(
        &self,
        intersection_id: u32,
        dir: Vec3,
        rails: &Query<&Rail>,
    ) -> f32 {
        let func = |min: f32, e: &Option<Entity>| {
            if let Some(e) = e {
                let rail = rails.get(*e).unwrap();

                let (start, end) =
                    if rail.joints[RAIL_START_JOINT].intersection_id == intersection_id {
                        (
                            rail.joints[RAIL_START_JOINT].pos,
                            rail.joints[RAIL_END_JOINT].pos,
                        )
                    } else {
                        (
                            rail.joints[RAIL_END_JOINT].pos,
                            rail.joints[RAIL_START_JOINT].pos,
                        )
                    };

                let rail_dir = (end - start).normalize();

                min.min(rail_dir.angle_between(dir))
            } else {
                min
            }
        };

        self.left.iter().chain(self.right.iter()).fold(90., func)
    }

    pub fn is_right_side(&self, pos: Vec3) -> bool {
        (pos - Into::<Vec3>::into(self.collision.center()))
            .normalize()
            .dot(self.right_forward)
            > 0.
    }

    pub fn get_nearest_forward(&self, pos: Vec3) -> Vec3 {
        if self.is_right_side(pos) {
            self.right_forward
        } else {
            -self.right_forward
        }
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
        start + start_forward * length * 0.5,
        end + end_forward * length * 0.5,
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

fn debug_rail_path(mut gizmos: Gizmos, q: Query<&Rail>) {
    q.into_iter().for_each(|rail| {
        // Draw line
        let points = create_curve_control_points(
            rail.joints[RAIL_START_JOINT].pos,
            rail.joints[RAIL_START_JOINT].forward,
            rail.joints[RAIL_END_JOINT].pos,
            rail.joints[RAIL_END_JOINT].forward,
        );

        let curve = CubicBezier::new(points).to_curve().unwrap();
        const STEPS: usize = 10;
        gizmos.linestrip(curve.iter_positions(STEPS), Color::WHITE);

        // Draw forwards
        gizmos.line(
            rail.joints[RAIL_START_JOINT].pos,
            rail.joints[RAIL_START_JOINT].pos + rail.joints[RAIL_START_JOINT].forward,
            Color::srgb(1.0, 0.1, 0.1),
        );
        gizmos.line(
            rail.joints[RAIL_START_JOINT].pos,
            rail.joints[RAIL_START_JOINT].pos + rail.joints[RAIL_START_JOINT].forward,
            Color::srgb(0.1, 0.1, 1.0),
        );
    });
}

fn debug_rail_intersections(
    intersections: Res<RailIntersections>,
    cursor: Single<&PlayerCursor>,
    mut gizmos: Gizmos,
    q: Query<&Rail>,
    mut contexts: EguiContexts,
) {
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    let collision = intersections.get_intersect_collision(&cursor_sphere);

    // Draw intersection info
    intersections.intersections.iter().for_each(|x| {
        gizmos.sphere(
            Isometry3d::from_translation(x.1.collision.center),
            x.1.collision.radius(),
            if collision.is_some_and(|y| y.0 == x.0) {
                Color::srgb(1.0, 0.0, 0.0)
            } else {
                Color::WHITE
            },
        );
    });

    // Print hovered intersection info
    if let Some(collision) = collision {
        egui::Window::new(format!("intersection {}", collision.0)).show(contexts.ctx_mut(), |ui| {
            ui.label(format!("{:#?}", collision.1));
        });

        // Mark the rails that are part of this intersection
        let get_rail_pos = |e: &Option<Entity>| {
            if let Some(e) = e {
                let rail = q.get(*e);
                if let Ok(rail) = rail {
                    let start = rail.joints[RAIL_START_JOINT].pos;
                    let end = rail.joints[RAIL_END_JOINT].pos;
                    Some((start, end))
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Draw connected rails
        collision.1.left.iter().for_each(|e| {
            if let Some((start, end)) = get_rail_pos(e) {
                gizmos.line(start, end, Color::srgb(1., 0., 0.));
            }
        });
        collision.1.right.iter().for_each(|e| {
            if let Some((start, end)) = get_rail_pos(e) {
                gizmos.line(start, end, Color::srgb(0., 1., 0.));
            }
        });

        // Draw right_forward
        let start: Vec3 = collision.1.collision.center.into();
        let end: Vec3 = start + collision.1.right_forward * 5.;
        gizmos.arrow(start, end, Color::srgb(0., 0., 1.));
    }
}
