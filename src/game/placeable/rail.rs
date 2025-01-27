use super::*;
use bevy::{
    math::bounding::{BoundingSphere, IntersectsVolume},
    picking::focus::PickingInteraction,
    utils::{hashbrown::HashSet, HashMap},
};
use bevy_egui::{egui, EguiContexts};
use bounding::BoundingVolume;
use cursor_feedback::CursorFeedback;

use crate::spline::*;
use rail_planner::*;

pub mod rail_graph;
pub mod rail_planner;

pub(super) fn rail_plugin(app: &mut App) {
    // app.add_systems(Update, (on_place_rail, debug_draw_rail_path));
    app.add_plugins((
        rail_graph::rail_graph_plugin,
        rail_planner::rail_planner_plugin,
    ));
    app.add_systems(Startup, load_rail_asset);
    app.add_systems(
        Update,
        (debug_rail_path, debug_rail_intersections).run_if(in_player_state(PlayerState::Building)),
    );
    app.init_resource::<RailIntersections>();
}

const RAIL_MIN_LENGTH: f32 = 10.;
const RAIL_MIN_DELTA_RADIANS: f32 = 15.0 * PI / 180.;
const RAIL_MAX_RADIANS: f32 = 22.5 * PI / 180.;
const RAIL_CURVES_MAX: usize = (PI / RAIL_MIN_DELTA_RADIANS) as usize;

#[derive(Resource)]
pub struct RailAsset {
    pub material: Handle<StandardMaterial>,
    pub hover_material: Handle<StandardMaterial>,
}

/// Contains the details to build and connect a rail
#[derive(Component)]
#[require(Spline, SplineMesh, Placeable(||Placeable::Rail), Name(|| Name::new("Rail")))]
pub struct Rail {
    pub joints: [RailJoint; 2],
}

impl Rail {
    fn new(
        self_entity: &mut EntityCommands,
        intersections: &mut ResMut<RailIntersections>,
        plan: &RailPlanner,
        spline: &Spline,
        rail_asset: &RailAsset,
    ) -> Rail {
        let start = spline.controls()[0].pos;
        let end = spline.controls()[1].pos;

        let start_intersection_id = plan.start_intersection_id.unwrap_or_else(|| {
            intersections.create_new_intersection(start, spline.controls()[0].forward)
        });

        let end_intersection_id = plan.end_intersection_id.unwrap_or_else(|| {
            intersections.create_new_intersection(end, -spline.controls()[1].forward)
        });

        let self_state = Rail {
            joints: [
                RailJoint {
                    intersection_id: start_intersection_id,
                },
                RailJoint {
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

        connect_intersection(
            self_entity.id(),
            &spline.controls()[0].forward,
            &mut start_intersection,
        );

        let mut end_intersection = intersections
            .intersections
            .get_mut(&end_intersection_id)
            .unwrap();
        connect_intersection(
            self_entity.id(),
            &spline.controls()[1].forward,
            &mut end_intersection,
        );

        self_entity
            .observe(update_material_on::<Pointer<Over>>(
                rail_asset.hover_material.clone(),
            ))
            .observe(update_material_on::<Pointer<Out>>(
                rail_asset.material.clone(),
            ))
            .observe(on_rail_destroy)
            .insert(MeshMaterial3d(rail_asset.material.clone()));

        self_state
    }

    pub fn destroy(
        &mut self,
        self_entity: Entity,
        c: &mut Commands,
        intersections: &mut ResMut<RailIntersections>,
    ) {
        let mut process = |intersection_id| {
            let intersection = intersections
                .intersections
                .get_mut(&intersection_id)
                .unwrap();
            if let Some(entry) = intersection
                .left
                .iter_mut()
                .chain(intersection.right.iter_mut())
                .find(|e| e.is_some_and(|e| e == self_entity))
            {
                *entry = None;
            }

            // Check if this intersection has no connections anymore, if so destroy it
            if intersection
                .left
                .iter()
                .chain(intersection.right.iter())
                .find(|e| e.is_some())
                .is_none()
            {
                intersections.intersections.remove(&intersection_id);
                intersections.id_provider.return_id(intersection_id);
            } else {
                intersection.left.sort_by(|a, b| b.cmp(a));
                intersection.right.sort_by(|a, b| b.cmp(a));
            }
        };

        process(self.joints[0].intersection_id);
        process(self.joints[1].intersection_id);
        c.entity(self_entity).despawn();
    }

    /// When we insert an intersection we remove the existing rail, split it into 2 and insert an intersection inbetween
    pub fn insert_intersection(
        &mut self,
        middle_intersection_id: u32,
        self_entity: Entity,
        pos: &Vec3,
        spline: &mut Spline,
        mut c: &mut Commands,
        mut intersections: &mut ResMut<RailIntersections>,
        rail_asset: &RailAsset,
        gizmos: &mut Option<&mut Gizmos>,
    ) {
        // Create a joint at the intersection point
        let default_plan = RailPlanner {
            start_intersection_id: None,
            end_intersection_id: None,
            is_initial_placement: false,
            status: RailPlannerStatus::Valid,
            start_rail: None,
            end_rail: None,
        };

        // Split start
        let (start_spline, end_spline) = spline.split(pos, gizmos);
        let mut start_entity = c.spawn_empty();
        let start_plan = RailPlanner {
            start_intersection_id: Some(self.joints[0].intersection_id),
            end_intersection_id: Some(middle_intersection_id),
            ..default_plan
        };
        let start_rail = Rail::new(
            &mut start_entity,
            &mut intersections,
            &start_plan,
            &start_spline,
            &rail_asset,
        );
        start_entity.insert((start_rail, start_spline));

        // Split end
        let mut end_entity = c.spawn_empty();
        let end_plan = RailPlanner {
            start_intersection_id: Some(middle_intersection_id),
            end_intersection_id: Some(self.joints[1].intersection_id),
            ..default_plan
        };
        let end_rail = Rail::new(
            &mut end_entity,
            &mut intersections,
            &end_plan,
            &end_spline,
            &rail_asset,
        );
        end_entity.insert((end_rail, end_spline));

        self.destroy(self_entity, &mut c, &mut intersections);
    }

    pub fn traverse(
        &self,
        t: f32,
        forward: &Dir3,
        distance: f32,
        spline: &Spline,
    ) -> TraverseResult {
        let right = spline.forward_from_t(t);
        let current_t = t;
        let delta_t = distance / spline.curve_length();

        let is_traveling_right = right.dot(forward.as_vec3()) > 0.0;
        if is_traveling_right {
            let new_t = current_t + delta_t;
            if new_t >= 1.0 {
                TraverseResult::Intersection {
                    t: 1.0,
                    pos: spline.controls()[1].pos,
                    forward: Dir3::new(-spline.controls()[1].forward).unwrap(),
                    remaining_distance: (distance - (delta_t * spline.curve_length())).max(0.0),
                    intersection_id: self.joints[1].intersection_id,
                }
            } else {
                let pos = spline.curve().sample(new_t).expect(
                    format!(
                        "{} is outside of domain curr {} delta {}",
                        new_t, current_t, delta_t
                    )
                    .as_str(),
                );
                TraverseResult::End {
                    t: new_t,
                    pos,
                    forward: spline.forward_from_t(new_t),
                }
            }
        } else {
            let new_t = current_t - delta_t;
            if new_t <= 0.0 {
                TraverseResult::Intersection {
                    t: 0.0,
                    pos: spline.controls()[0].pos,
                    forward: Dir3::new(-spline.controls()[0].forward).unwrap(),
                    remaining_distance: (distance - (delta_t * spline.curve_length())).max(0.0),
                    intersection_id: self.joints[0].intersection_id,
                }
            } else {
                let pos = spline.curve().sample(new_t).expect(
                    format!(
                        "{} is outside of domain curr {} delta {}",
                        new_t, current_t, delta_t
                    )
                    .as_str(),
                );
                TraverseResult::End {
                    t: new_t,
                    pos,
                    forward: -spline.forward_from_t(new_t),
                }
            }
        }
    }
}

pub enum TraverseResult {
    /// We have finished calculating our position if we would traverse
    End { t: f32, pos: Vec3, forward: Dir3 },
    /// We have reached the end of the spline, and can
    Intersection {
        t: f32,
        pos: Vec3,
        forward: Dir3,
        remaining_distance: f32,
        intersection_id: u32,
    },
}
/// Represents the data for the rail end points
pub struct RailJoint {
    pub intersection_id: u32,
}

#[derive(Resource, Default)]
pub struct RailIntersections {
    pub intersections: HashMap<u32, RailIntersection>,
    pub id_provider: IdProvider,
}

impl RailIntersections {
    pub fn get_connected_intersections(
        &self,
        intersection_id: u32,
        rails: &Query<&Rail>,
    ) -> Vec<u32> {
        let mut collect = HashSet::new();
        self.collect_connected_intersections(intersection_id, rails, &mut collect);
        collect.into_iter().collect()
    }

    fn collect_connected_intersections(
        &self,
        intersection_id: u32,
        rails: &Query<&Rail>,
        collect: &mut HashSet<u32>,
    ) {
        collect.insert(intersection_id);
        let root = self.intersections.get(&intersection_id).unwrap();
        root.left.iter().chain(root.right.iter()).for_each(|e| {
            if let Some(e) = e {
                let rail = rails.get(*e).unwrap();
                if !collect.contains(&rail.joints[0].intersection_id) {
                    self.collect_connected_intersections(
                        rail.joints[0].intersection_id,
                        rails,
                        collect,
                    );
                }
                if !collect.contains(&rail.joints[1].intersection_id) {
                    self.collect_connected_intersections(
                        rail.joints[1].intersection_id,
                        rails,
                        collect,
                    );
                }
            }
        });
    }

    pub fn get_intersect_collision(
        &self,
        sphere: &BoundingSphere,
    ) -> Option<(&u32, &RailIntersection)> {
        self.intersections
            .iter()
            .find(|x| x.1.collision.intersects(sphere))
    }

    pub fn create_new_intersection(&mut self, pos: Vec3, right_forward: Vec3) -> u32 {
        const SIZE: f32 = 2.5;
        let idx = self.id_provider.get_available_id();
        self.intersections.insert(
            idx,
            RailIntersection {
                right_forward,
                left: [None; RAIL_CURVES_MAX],
                right: [None; RAIL_CURVES_MAX],
                collision: BoundingSphere::new(pos, SIZE),
            },
        );

        idx
    }
}

/// Can be considered as a node in a graph
/// A junction is supported by inserting an intersection
/// Traffic control is controlled by inserting an intersection, to split traffic groups
#[derive(Debug)]
pub struct RailIntersection {
    pub left: [Option<Entity>; RAIL_CURVES_MAX],
    pub right: [Option<Entity>; RAIL_CURVES_MAX],
    /// The "right" forward decided whether the rail will be put in the left or right group.
    /// When traversing the rails we know if we can go left or right by aligning our
    /// incoming dir with the right_forward dir
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
        rails: &Query<(&Rail, &Spline)>,
    ) -> f32 {
        let func = |min: f32, e: &Option<Entity>| {
            if let Some(e) = e {
                let (rail, spline) = rails.get(*e).unwrap();

                let (start, end) = if rail.joints[0].intersection_id == intersection_id {
                    (spline.controls()[0].pos, spline.controls()[1].pos)
                } else {
                    (spline.controls()[1].pos, spline.controls()[0].pos)
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

    pub fn get_curve_options(&self, forward: &Dir3) -> Vec<Entity> {
        if forward.dot(self.right_forward) > 0.0 {
            self.right.iter().filter_map(|x| *x).collect()
        } else {
            self.left.iter().filter_map(|x| *x).collect()
        }
    }
}

fn on_rail_destroy(
    trigger: Trigger<DestroyEvent>,
    mut q: Query<&mut Rail>,
    trains: Query<&Train>,
    mut c: Commands,
    mut intersections: ResMut<RailIntersections>,
    mut feedback: ResMut<CursorFeedback>,
) {
    // Check if there are no trains on this rail
    let entity = trigger.entity();
    for train in trains.iter() {
        if train.rail == entity {
            feedback.entries.push(
                CursorFeedbackData::default()
                    .with_error("Trains are on rail".into())
                    .with_duration(3.0),
            );
            return;
        }
    }

    q.get_mut(entity)
        .unwrap()
        .destroy(trigger.entity(), &mut c, &mut intersections);
}

fn load_rail_asset(mut c: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    c.insert_resource(RailAsset {
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.3),
            ..default()
        }),
        hover_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.5),
            ..default()
        }),
    });
}

fn debug_rail_path(
    mut gizmos: Gizmos,
    q: Query<(&Spline, Option<&PickingInteraction>), With<Rail>>,
) {
    q.into_iter().for_each(|(spline, picking)| {
        // Draw line
        let curve = spline.curve();
        const STEPS: usize = 10;
        gizmos.linestrip(curve.iter_positions(STEPS), Color::WHITE);

        // Draw forwards
        gizmos.line(
            spline.controls()[0].pos,
            spline.controls()[0].pos + spline.controls()[0].forward,
            Color::srgb(1.0, 0.1, 0.1),
        );
        gizmos.line(
            spline.controls()[0].pos,
            spline.controls()[0].pos + spline.controls()[0].forward,
            Color::srgb(0.1, 0.1, 1.0),
        );

        if picking.is_some_and(|x| x == &PickingInteraction::Hovered) {
            gizmos.linestrip(
                spline.create_curve_control_points()[0],
                Color::srgb(0.5, 0.5, 0.5),
            );
        }
    });
}

fn debug_rail_intersections(
    intersections: Res<RailIntersections>,
    cursor: Single<&PlayerCursor>,
    mut gizmos: Gizmos,
    q: Query<&Spline, With<Rail>>,
    mut contexts: EguiContexts,
) {
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    let collision = intersections.get_intersect_collision(&cursor_sphere);

    // Mark the rails that are part of this intersection
    let get_rail_pos = |e: &Option<Entity>| {
        if let Some(e) = e {
            let spline = q.get(*e);
            if let Ok(spline) = spline {
                let start = spline.controls()[0].pos;
                let end = spline.controls()[1].pos;
                Some((start, end))
            } else {
                None
            }
        } else {
            None
        }
    };

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

        // // Draw connected rails
        // x.1.left.iter().for_each(|e| {
        //     if let Some((start, end)) = get_rail_pos(e) {
        //         let side = Quat::from_rotation_y(FRAC_PI_2) * (start - end).normalize() * 2.5;
        //         gizmos.line(start + side, end, Color::srgb(0., 1., 0.));
        //     }
        // });
        // x.1.right.iter().for_each(|e| {
        //     if let Some((start, end)) = get_rail_pos(e) {
        //         let side = Quat::from_rotation_y(FRAC_PI_2) * (start - end).normalize() * 2.5;
        //         gizmos.line(start - side, end, Color::srgb(1., 0., 0.));
        //     }
        // });

        // // Draw right_forward
        // let start: Vec3 = x.1.collision.center.into();
        // let end: Vec3 = start + x.1.right_forward * 5.;
        // gizmos.arrow(start, end, Color::srgb(0., 0., 1.));
    });

    // Print hovered intersection info
    if let Some(collision) = collision {
        egui::Window::new(format!("intersection {}", collision.0)).show(contexts.ctx_mut(), |ui| {
            ui.label(format!("{:#?}", collision.1));
        });

        // Draw connected rails
        collision.1.left.iter().for_each(|e| {
            if let Some((start, end)) = get_rail_pos(e) {
                gizmos.line(start + Vec3::Y, end + Vec3::Y, Color::srgb(0., 1., 0.));
            }
        });
        collision.1.right.iter().for_each(|e| {
            if let Some((start, end)) = get_rail_pos(e) {
                gizmos.line(start + Vec3::Y, end + Vec3::Y, Color::srgb(1., 0., 0.));
            }
        });

        // Draw right_forward
        let start: Vec3 = collision.1.collision.center.into();
        let end: Vec3 = start + collision.1.right_forward * 5.;
        gizmos.arrow(start, end, Color::srgb(0., 0., 1.));
    }
}
