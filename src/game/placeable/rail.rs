use super::*;
use bevy::{
    math::bounding::{BoundingSphere, IntersectsVolume},
    picking::{focus::PickingInteraction, mesh_picking::ray_cast::RayMeshHit},
    utils::{hashbrown::HashSet, HashMap},
};
use bounding::BoundingVolume;
use cursor_feedback::CursorFeedback;
use uuid::Uuid;

use crate::spline::*;
use rail_planner::*;

pub mod rail_graph;
pub mod rail_planner;

pub(super) fn rail_plugin(app: &mut App) {
    app.add_plugins((
        rail_graph::rail_graph_plugin,
        rail_planner::rail_planner_plugin,
    ));
    app.add_event::<RailIntersectionChangedEvent>();
    app.add_event::<RailIntersectionRemovedEvent>();
    app.add_event::<RailRemovedEvent>();
    app.add_systems(Startup, load_rail_asset);
    app.add_systems(
        Update,
        (
            on_rail_mesh_changed,
            (debug_rail_path, debug_rail_intersections)
                .run_if(in_player_state(PlayerState::Building)),
        ),
    );
    app.init_resource::<RailIntersections>();
}

pub const RAIL_MIN_LENGTH: f32 = 10.;
pub const RAIL_SEGMENT_LENGTH: f32 = 1.0;
pub const RAIL_SEGMENT_WIDTH: f32 = 1.435;
pub const RAIL_SEGMENT_HEIGHT: f32 = 0.20;
pub const RAIL_MIN_DELTA_RADIANS: f32 = 15.0 * PI / 180.;
pub const RAIL_MAX_RADIANS: f32 = 10. * PI / 180.;
pub const RAIL_CURVES_MAX: usize = (PI / RAIL_MIN_DELTA_RADIANS) as usize;

#[derive(Event, PartialEq, PartialOrd, Eq, Ord)]
pub struct RailIntersectionChangedEvent(Uuid);

#[derive(Event)]
pub struct RailIntersectionRemovedEvent(RailIntersection);

#[derive(Event)]
pub struct RailRemovedEvent(Entity);

#[derive(Event, Resource)]
pub struct RailAsset {
    pub material: Handle<StandardMaterial>,
    pub hover_material: Handle<StandardMaterial>,
    pub segment: Handle<Scene>,
}

fn create_rail_spline() -> Spline {
    Spline::default()
        .with_min_segment_length(RAIL_SEGMENT_LENGTH)
        .with_height(RAIL_SEGMENT_HEIGHT)
}

/// Contains the details to build and connect a rail
#[derive(Component)]
#[require(
    Spline(create_rail_spline),
    SplineMesh(|| SplineMesh::default().with_width(RAIL_SEGMENT_WIDTH)),
    Placeable(||Placeable::Rail),
    GenerateCollider,
    Name(|| Name::new("Rail"))
)]
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

                side[RailIntersection::empty_idx(side).unwrap()] = Some(entity);
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
            // .observe(update_material_on::<Pointer<Over>>(
            //     rail_asset.hover_material.clone(),
            // ))
            // .observe(update_material_on::<Pointer<Out>>(
            //     rail_asset.material.clone(),
            // ))
            .observe(on_rail_destroy)
            .insert(MeshMaterial3d(rail_asset.material.clone()));

        self_state
    }

    pub fn far_intersection<'a>(
        &self,
        pos: &Vec3,
        intersections: &'a RailIntersections,
    ) -> &'a RailIntersection {
        let start = intersections
            .intersections
            .get(&self.joints[0].intersection_id)
            .unwrap();
        let end = intersections
            .intersections
            .get(&self.joints[1].intersection_id)
            .unwrap();

        if pos.distance_squared(start.collision.center.into())
            < pos.distance_squared(end.collision.center.into())
        {
            end
        } else {
            start
        }
    }

    pub fn destroy(
        &mut self,
        self_entity: Entity,
        c: &mut Commands,
        intersections: &mut ResMut<RailIntersections>,
        ev_rail_removed: &mut EventWriter<RailRemovedEvent>,
        ev_intersection_removed: &mut EventWriter<RailIntersectionRemovedEvent>,
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
                ev_intersection_removed.send(RailIntersectionRemovedEvent(intersection.clone()));
                intersections.intersections.remove(&intersection_id);
            } else {
                intersection.left.sort_by(|a, b| b.cmp(a));
                intersection.right.sort_by(|a, b| b.cmp(a));
            }
        };

        process(self.joints[0].intersection_id);
        process(self.joints[1].intersection_id);

        ev_rail_removed.send(RailRemovedEvent(self_entity));
        c.entity(self_entity).despawn_recursive();
    }

    /// When we insert an intersection we remove the existing rail, split it into 2 and insert an intersection inbetween
    pub fn insert_intersection(
        &mut self,
        middle_intersection_id: Uuid,
        self_entity: Entity,
        pos: &Vec3,
        spline: &mut Spline,
        mut c: &mut Commands,
        mut intersections: &mut ResMut<RailIntersections>,
        rail_asset: &RailAsset,
        modified_intersection_ids: &mut Vec<Uuid>,
        mut ev_rail_removed: &mut EventWriter<RailRemovedEvent>,
        mut ev_intersection_removed: &mut EventWriter<RailIntersectionRemovedEvent>,
        gizmos: Option<&mut Gizmos>,
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

        modified_intersection_ids.extend([
            self.joints[0].intersection_id,
            middle_intersection_id,
            self.joints[1].intersection_id,
        ]);

        self.destroy(
            self_entity,
            &mut c,
            &mut intersections,
            &mut ev_rail_removed,
            &mut ev_intersection_removed,
        );
    }

    pub fn traverse(
        &self,
        t: f32,
        forward: &Dir3,
        remaining_distance: f32,
        spline: &Spline,
    ) -> TraverseResult {
        let right = spline.forward(t);
        let is_traveling_right = right.dot(forward.as_vec3()) > 0.0;
        let new_t = spline.traverse(
            t,
            if is_traveling_right {
                remaining_distance
            } else {
                -remaining_distance
            },
        );

        if new_t >= 1.0 {
            TraverseResult::Intersection {
                t: 1.0,
                pos: spline.controls()[1].pos + Vec3::Y * spline.height,
                forward: -spline.controls()[1].forward,
                // Move it by a little bit, otherwise during our next iter
                // we will be considered on an intersection again
                remaining_distance: remaining_distance * 0.5,
                intersection_id: self.joints[1].intersection_id,
            }
        } else if new_t <= 0.0 {
            TraverseResult::Intersection {
                t: 0.0,
                pos: spline.controls()[0].pos + Vec3::Y * spline.height,
                forward: -spline.controls()[0].forward,
                remaining_distance: remaining_distance * 0.5,
                intersection_id: self.joints[0].intersection_id,
            }
        } else {
            TraverseResult::End {
                t: new_t,
                pos: spline.projected_position(new_t),
                forward: if is_traveling_right {
                    spline.forward(new_t)
                } else {
                    -spline.forward(new_t)
                },
            }
        }
    }
}

pub enum TraverseResult {
    /// We have finished calculating our position if we would traverse
    End { t: f32, pos: Vec3, forward: Dir3 },
    /// We have reached the end of the spline
    Intersection {
        /// T of the current spline, if we are at end, it will be 1
        t: f32,
        pos: Vec3,
        forward: Dir3,
        remaining_distance: f32,
        /// The ID of the intersection we're at
        intersection_id: Uuid,
    },
}

/// Represents the data for the rail end points
#[derive(Clone, Copy)]
pub struct RailJoint {
    pub intersection_id: Uuid,
}

#[derive(Resource, Default)]
pub struct RailIntersections {
    pub intersections: HashMap<Uuid, RailIntersection>,
}

impl RailIntersections {
    pub fn get_connected_intersections(
        &self,
        intersection_id: Uuid,
        rails: &Query<&Rail>,
    ) -> Vec<Uuid> {
        let mut collect = HashSet::new();
        self.collect_connected_intersections(intersection_id, rails, &mut collect);
        collect.into_iter().collect()
    }

    fn collect_connected_intersections(
        &self,
        intersection_id: Uuid,
        rails: &Query<&Rail>,
        collect: &mut HashSet<Uuid>,
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
    ) -> Option<(&Uuid, &RailIntersection)> {
        self.intersections
            .iter()
            .find(|x| x.1.collision.intersects(sphere))
    }

    pub fn create_new_intersection(&mut self, pos: Vec3, right_forward: Dir3) -> Uuid {
        let uuid = Uuid::new_v4();
        let intersection = RailIntersection::new(uuid, pos, right_forward);

        // graph.add_intersection(&intersection);
        self.intersections.insert(uuid, intersection);

        uuid
    }
}

/// Can be considered as a node in a graph
/// A junction is supported by inserting an intersection
/// Traffic control is controlled by inserting an intersection, to split traffic groups
#[derive(Debug, Clone, Copy)]
pub struct RailIntersection {
    pub uuid: Uuid,
    pub left: [Option<Entity>; RAIL_CURVES_MAX],
    pub right: [Option<Entity>; RAIL_CURVES_MAX],
    /// The "right" forward decided whether the rail will be put in the left or right group.
    /// When traversing the rails we know if we can go left or right by aligning our
    /// incoming dir with the right_forward dir
    pub right_forward: Dir3,
    pub collision: BoundingSphere,
}

impl RailIntersection {
    pub fn new(uuid: Uuid, pos: Vec3, right_forward: Dir3) -> Self {
        const SIZE: f32 = RAIL_SEGMENT_WIDTH * 0.75;

        RailIntersection {
            uuid,
            right_forward,
            left: [None; RAIL_CURVES_MAX],
            right: [None; RAIL_CURVES_MAX],
            collision: BoundingSphere::new(pos, SIZE),
        }
    }

    pub fn empty_idx(intersections: &[Option<Entity>; RAIL_CURVES_MAX]) -> Option<usize> {
        intersections
            .iter()
            .enumerate()
            .find(|(_, v)| v.is_none())
            .and_then(|(i, _)| Some(i))
    }

    pub fn min_angle_relative_to_others(
        &self,
        intersection_id: Uuid,
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
            .dot(self.right_forward.as_vec3())
            > 0.
    }

    pub fn nearest_forward(&self, pos: Vec3) -> Dir3 {
        if self.is_right_side(pos) {
            self.right_forward
        } else {
            -self.right_forward
        }
    }

    pub fn curves(&self) -> Vec<Entity> {
        self.left
            .iter()
            .chain(self.right.iter())
            .filter_map(|x| *x)
            .collect()
    }

    pub fn curve_options(&self, forward: &Dir3) -> Vec<Entity> {
        if forward.dot(self.right_forward.as_vec3()) > 0.0 {
            self.right.iter().filter_map(|x| *x).collect()
        } else {
            self.left.iter().filter_map(|x| *x).collect()
        }
    }
}

fn on_rail_mesh_changed(
    mut c: Commands,
    q: Query<
        (Entity, &SplineMesh, &Spline),
        (Or<(With<Rail>, With<RailPlanner>)>, Changed<SplineMesh>),
    >,
    asset: Res<RailAsset>,
    mut meshes: ResMut<Assets<Mesh>>,
    rail_asset: Res<RailAsset>,
) {
    q.iter().for_each(|(e, mesh, spline)| {
        let mut ec = c.entity(e);
        ec.despawn_descendants();

        ec.with_children(|parent| {
            const SIZE: f32 = 0.12;
            const Y_OFFSET: f32 = 0.08;
            const DISTANCE: f32 = 0.5;
            for i in [-DISTANCE, DISTANCE] {
                let mut mesh = SplineMesh::create_mesh();

                let buffers = SplineMesh::create_buffers(&spline, SIZE, SIZE, vec2(i, Y_OFFSET));
                // SplineMesh::create_buffers(&spline, SIZE, SIZE + Y_OFFSET, vec2(i, 0.0));

                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.0);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, buffers.1);
                mesh.insert_indices(buffers.2);

                parent.spawn((
                    Mesh3d(meshes.add(mesh)),
                    Visibility::Visible,
                    MeshMaterial3d(rail_asset.material.clone()),
                ));
            }

            spline.curve_points().iter().for_each(|pos| {
                let t = spline.t_from_pos(pos);
                let forward = spline.forward(t);
                parent.spawn((
                    SceneRoot(asset.segment.clone()),
                    Transform::from_translation(*pos)
                        .with_scale(Vec3::new(mesh.width, mesh.width, mesh.width))
                        .with_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, forward.as_vec3())),
                    Visibility::Visible,
                ));
            });
        });
    });
}

pub fn get_closest_rail(
    ray: Ray3d,
    ray_cast: &mut MeshRayCast,
    query: &Query<&Spline, With<Rail>>,
) -> Option<(Entity, RayMeshHit)> {
    let hits = ray_cast
        .cast_ray(
            ray,
            &RayCastSettings::default()
                .with_visibility(RayCastVisibility::Any)
                .with_filter(&|x| query.contains(x))
                .never_early_exit(),
        )
        .to_owned();
    if hits.is_empty() {
        return None;
    }

    let hit = hits
        .into_iter()
        .min_by(|x, y| {
            let a = query.get(x.0).unwrap();
            let b = query.get(y.0).unwrap();
            let a = a
                .projected_position(a.t_from_pos(&x.1.point))
                .distance_squared(x.1.point);
            let b = b
                .projected_position(b.t_from_pos(&y.1.point))
                .distance_squared(y.1.point);
            a.total_cmp(&b)
        })
        .unwrap();

    Some(hit)
}

fn on_rail_destroy(
    trigger: Trigger<DestroyEvent>,
    mut q: Query<&mut Rail>,
    trains: Query<&Train>,
    mut c: Commands,
    mut intersections: ResMut<RailIntersections>,
    mut feedback: ResMut<CursorFeedback>,
    mut ev_rail_removed: EventWriter<RailRemovedEvent>,
    mut ev_intersection_removed: EventWriter<RailIntersectionRemovedEvent>,
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

    q.get_mut(entity).unwrap().destroy(
        trigger.entity(),
        &mut c,
        &mut intersections,
        &mut ev_rail_removed,
        &mut ev_intersection_removed,
    );
}

fn load_rail_asset(
    mut c: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    c.insert_resource(RailAsset {
        material: materials.add(StandardMaterial {
            base_color: Color::srgb_u8(189, 197, 237),
            // Causes odd shadows issues
            // double_sided: true,
            cull_mode: None,
            ..default()
        }),
        hover_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.5),
            ..default()
        }),
        segment: asset_server
            .load(GltfAssetLabel::Scene(0).from_asset("models/spline_segment.glb")),
    });
}

fn debug_rail_path(
    mut gizmos: Gizmos,
    q: Query<(&Spline, Option<&PickingInteraction>), With<Rail>>,
) {
    q.into_iter().for_each(|(spline, picking)| {
        // Draw line
        gizmos.linestrip(spline.curve_points_projected().clone(), Color::WHITE);

        // Draw forwards
        gizmos.line(
            spline.controls()[0].pos,
            spline.controls()[0].pos + spline.controls()[0].forward.as_vec3(),
            Color::srgb(1.0, 0.1, 0.1),
        );
        gizmos.line(
            spline.controls()[0].pos,
            spline.controls()[0].pos + spline.controls()[0].forward.as_vec3(),
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
    // mut contexts: EguiContexts,
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
        // egui::Window::new("intersection").show(contexts.ctx_mut(), |ui| {
        //     ui.label(format!("{:#?}", collision.1));
        // });

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
