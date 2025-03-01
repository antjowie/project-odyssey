//! Logic responsible for generating a preview of what RailBuilding will be built
use super::*;
use bevy::math::vec3;
use cursor_feedback::*;
use std::f32::consts::FRAC_PI_2;

pub fn rail_planner_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            (
                update_rail_planner_feedback.run_if(any_with_component::<RailPlanner>),
                draw_rail_planner,
            )
                .in_set(GameSet::Update),
            (
                update_intitial_rail_planner.run_if(not(any_with_component::<RailPlanner>)),
                update_rail_planner,
            )
                .in_set(GameSet::Spawn),
        )
            .run_if(
                // We use any with compnent as we assume it exists on some of the funcs
                any_with_component::<InputContext<PlayerBuildAction>>
                    .and(is_placeable_preview(Placeable::Rail)),
            ),
    );
}

#[derive(Component)]
#[require(
    Name(|| Name::new("RailPlanner")),
    Spline(create_rail_spline),
    SplineMesh(|| SplineMesh::default().with_width(RAIL_SEGMENT_WIDTH)),
    NotShadowCaster
)]
pub struct RailPlanner {
    pub start_intersection_id: Option<Uuid>,
    /// Initial placement is used together with the presence of a start joint
    /// If we have no initial placement, we want the first placement to confirm
    /// the start_forward orientation
    pub is_initial_placement: bool,
    pub end_intersection_id: Option<Uuid>,
    pub status: RailPlannerStatus,

    /// If start or end rail are defined we want to split them, since we are
    /// creating an intersection somewhere on the existing rail
    pub start_rail: Option<Entity>,
    pub end_rail: Option<Entity>,
}

impl RailPlanner {
    fn new(start_pos: Vec3, spline: &mut Spline) -> Self {
        spline.set_controls([
            SplineControl {
                pos: start_pos,
                forward: Dir3::new_unchecked(Vec3::NEG_Z),
            },
            SplineControl {
                pos: start_pos,
                forward: Dir3::new_unchecked(Vec3::NEG_Z),
            },
        ]);

        RailPlanner {
            start_intersection_id: None,
            is_initial_placement: true,
            end_intersection_id: None,
            status: RailPlannerStatus::Valid,
            start_rail: None,
            end_rail: None,
        }
    }

    fn is_initial_placement(&self) -> bool {
        self.is_initial_placement
            && self.start_intersection_id.is_none()
            && self.start_rail.is_none()
    }
}

#[derive(Default, PartialEq, Debug, Copy, Clone)]
pub enum RailPlannerStatus {
    #[default]
    Valid,
    CurveTooSharp(f32),
    /// Our delta angle is too close to any other curves in our joint
    CurveTooShallow(f32),
    RailTooShort(f32),
    /// TODO: Something we might wanna support, but atm we are storing entity references
    /// so removing it and then readding it, while possible is such an edge case I don't feel like
    /// implementing it atm
    ExtendIntoSelf,
    ExtendTooCloseToIntersection(f32),
    /// TODO: We can support this by making sure trains during this modification
    /// are reassigned to rails, but for now we just disallow it
    TrainOnStartRail,
    TrainOnEndRail,
}

fn handle_build_state_cancel_event(
    mut trigger: Trigger<BuildStateCancelEvent>,
    mut q: Query<&mut RailPlanner>,
    mut c: Commands,
) {
    trigger.propagate(false);
    let mut plan = q.get_mut(trigger.entity()).unwrap();

    // Our initial placement is 2 steps. If we are in 2nd step and cancel
    // we simply go back to first step
    if plan.start_intersection_id.is_none() && !plan.is_initial_placement {
        plan.is_initial_placement = true;
    } else {
        c.entity(trigger.entity()).despawn_recursive();
    }
}

fn update_intitial_rail_planner(
    mut c: Commands,
    q: Query<Entity, With<RailPlanner>>,
    player_state: Query<(Entity, &PlayerCursor, &ActionState<PlayerBuildAction>)>,
    intersections: Res<RailIntersections>,
    mut event: EventReader<PlayerStateChangedEvent>,
    asset: Res<RailAsset>,
    mut ray_cast: MeshRayCast,
    mut rails: Query<(Entity, &Rail, &Spline), Without<PlaceablePreview>>,
    mut align_to_right: Local<bool>,
    mut gizmos: Gizmos,
    mut feedback: ResMut<CursorFeedback>,
) {
    // Hacky, but we want to ignore placing this on the switch to view mode
    for ev in event.read() {
        if ev.new_state == PlayerState::Building && ev.old_state == PlayerState::Viewing {
            return;
        }
    }

    let (state_e, cursor, input) = player_state.single();
    let mut spline = create_rail_spline();
    let mut plan = RailPlanner::new(cursor.build_pos, &mut spline);
    let mut pos = cursor.build_pos;

    // Check if we have an intersection with an intersection
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);
    plan.start_intersection_id = intersections
        .get_intersect_collision(&cursor_sphere)
        .and_then(|x| {
            pos = x.1.collision.center.into();
            spline.set_controls_index(
                0,
                SplineControl {
                    pos,
                    forward: x.1.nearest_forward(cursor.world_pos),
                },
            );

            let start: Vec3 = x.1.collision.center.into();
            let end = start + x.1.nearest_forward(cursor.world_pos) * 10.;

            gizmos
                .arrow(start, end, Color::srgb(1., 1., 0.))
                .with_tip_length(5.);

            Some(*x.0)
        });

    // Check if we have an intersection with a mesh
    plan.start_rail = None;
    if plan.start_intersection_id.is_none() {
        let hit = get_closest_rail(
            cursor.ray,
            &mut ray_cast,
            &rails
                .transmute_lens_filtered::<&Spline, With<Rail>>()
                .query(),
        );
        if let Some(hit) = hit {
            if let Ok((e, target_rail, target_spline)) = rails.get(hit.0) {
                plan.start_rail = Some(e);

                let t = target_spline.t_from_pos(&cursor.build_pos);
                let spline_pos = target_spline.position(t);
                let mut forward = target_spline.forward(t);

                pos = spline_pos;

                // Check if we are trying to expand rail
                if input.just_pressed(&PlayerBuildAction::Rotate) {
                    *align_to_right = !*align_to_right;
                }

                if *align_to_right == false {
                    forward = -forward;
                }

                let start = intersections
                    .intersections
                    .get(&target_rail.joints[0].intersection_id)
                    .unwrap()
                    .collision
                    .center;
                let end = intersections
                    .intersections
                    .get(&target_rail.joints[1].intersection_id)
                    .unwrap()
                    .collision
                    .center;
                let min_distance = pos.distance(start.into()).min(pos.distance(end.into()));

                if min_distance < RAIL_MIN_LENGTH {
                    plan.status = RailPlannerStatus::ExtendTooCloseToIntersection(min_distance);
                    feedback.entries.push(
                        CursorFeedbackData::default()
                            .with_error("Too close to intersection".into()),
                    );
                }

                gizmos
                    .arrow(pos, pos + forward * 10.0, Color::srgb_u8(200, 100, 100))
                    .with_tip_length(5.0);

                spline.set_controls_index(0, SplineControl { pos, forward });
            }
        }

        gizmos.cuboid(
            Transform::from_translation(pos).with_scale(Vec3::splat(2.0)),
            Color::WHITE,
        );
    }
    if q.is_empty()
        && plan.status == RailPlannerStatus::Valid
        && input.just_pressed(&PlayerBuildAction::Interact)
    {
        c.spawn(plan)
            .insert(spline)
            .insert(Placeable::Rail)
            .insert(PlaceablePreview::new(state_e))
            .insert(MeshMaterial3d(asset.material.clone()))
            .observe(handle_build_state_cancel_event);
    }
}

fn update_rail_planner(
    mut gizmos: Gizmos,
    mut c: Commands,
    mut q: Query<(&mut RailPlanner, &mut Spline), Without<Rail>>,
    mut rails: Query<(Entity, &mut Rail, &mut Spline), Without<PlaceablePreview>>,
    trains: Query<&Train>,
    mut player_states: Query<(&mut PlayerCursor, &ActionState<PlayerBuildAction>)>,
    mut intersections: ResMut<RailIntersections>,
    asset: Res<RailAsset>,
    mut ray_cast: MeshRayCast,
    // prev_hover, should_align
    mut align_to_right: Local<(bool, bool)>,
    mut ev_changed: EventWriter<RailIntersectionChangedEvent>,
    mut ev_rail_removed: EventWriter<RailRemovedEvent>,
    mut ev_intersection_removed: EventWriter<RailIntersectionRemovedEvent>,
) {
    let (mut cursor, input) = player_states.single_mut();

    q.iter_mut().for_each(|(mut plan, mut spline)| {
        let mut controls = spline.controls().clone();
        controls[1].pos = cursor.build_pos;

        let mut delta = controls[1].pos - controls[0].pos;
        let length = delta.length();
        if length > RAIL_MAX_LENGTH {
            controls[1].pos = controls[0].pos + delta.normalize() * RAIL_MAX_LENGTH;
            delta = controls[1].pos - controls[0].pos;
        }

        let to_end = Dir3::new(delta).unwrap_or(Dir3::new_unchecked(Vec3::NEG_Z));
        plan.status = RailPlannerStatus::Valid;

        // This is our initial placement, we don't have an orientation yet
        if plan.is_initial_placement() {
            controls[0].forward = to_end;
            controls[1].forward = -to_end;
            plan.end_intersection_id = None;
            plan.end_rail = None;

            // If we have no orientation, we get no forward
            if controls[0].pos != controls[1].pos
                && input.just_pressed(&PlayerBuildAction::Interact)
            {
                plan.is_initial_placement = false;
            }
        } else {
            // Calculate forwards which control curve shape
            match cursor.curve_mode {
                PathCurveMode::Curve => {
                    let incidence = controls[0].forward;
                    let towards_2d = Vec2::from_angle(FRAC_PI_2).rotate(to_end.xz());
                    let normal = vec3(towards_2d.x, 0., towards_2d.y);
                    gizmos.line(controls[0].pos, controls[0].pos + normal, Color::BLACK);
                    controls[1].forward = Dir3::new(-incidence.reflect(normal)).unwrap();
                }
                PathCurveMode::Straight => {
                    controls[1].forward = -controls[0].forward;
                }
                PathCurveMode::Chase => {
                    controls[1].forward = -to_end;
                }
            }
            controls[1].forward =
                Quat::from_rotation_y(cursor.manual_rotation) * controls[1].forward;

            // Check if we hover over another intersection, if so we align our end_forward
            plan.end_rail = None;
            plan.end_intersection_id = None;
            let sphere = BoundingSphere::new(cursor.world_pos, 0.1);
            let intersection = intersections.get_intersect_collision(&sphere);
            if intersection.is_some() {
                let x = intersection.unwrap();
                if plan.start_intersection_id.is_none_or(|y| y != *x.0) {
                    controls[1].pos = x.1.collision.center.into();
                    controls[1].forward = x.1.nearest_forward(cursor.world_pos);
                    plan.end_intersection_id = Some(*x.0);
                    *align_to_right = (false, false);
                }
            } else {
                // Check if we hover over another rail, if so we insert intersection
                let hit = get_closest_rail(
                    cursor.ray,
                    &mut ray_cast,
                    &rails
                        .transmute_lens_filtered::<&Spline, With<Rail>>()
                        .query(),
                );
                if let Some(hit) = hit {
                    if let Ok((entity, target_rail, target_spline)) = rails.get(hit.0) {
                        plan.end_rail = Some(entity);

                        let t = target_spline.t_from_pos(&cursor.build_pos);
                        let pos = target_spline.position(t);
                        let mut forward = target_spline.forward(t);
                        controls[1].pos = pos;

                        if align_to_right.0 == false {
                            if forward.dot(controls[1].forward.as_vec3()) > 0.0 {
                                align_to_right.1 = true;
                            }
                        }
                        align_to_right.0 = true;

                        if input.just_pressed(&PlayerBuildAction::Rotate) {
                            align_to_right.1 = !align_to_right.1;
                        }

                        if align_to_right.1 == false {
                            forward = -forward;
                        }

                        target_spline.split(&pos, Some(&mut gizmos));
                        controls[1].forward = forward;

                        let start = intersections
                            .intersections
                            .get(&target_rail.joints[0].intersection_id)
                            .unwrap()
                            .collision
                            .center;
                        let end = intersections
                            .intersections
                            .get(&target_rail.joints[1].intersection_id)
                            .unwrap()
                            .collision
                            .center;
                        let min_distance = pos.distance(start.into()).min(pos.distance(end.into()));

                        if min_distance < RAIL_MIN_LENGTH {
                            plan.status =
                                RailPlannerStatus::ExtendTooCloseToIntersection(min_distance);
                        }
                    }
                } else {
                    *align_to_right = (false, false);
                }
            }

            // Validate our plan, if it's still valid all former checks have passed
            if plan.status == RailPlannerStatus::Valid {
                let start_min_angle = if let Some(id) = plan.start_intersection_id {
                    intersections
                        .intersections
                        .get(&id)
                        .unwrap()
                        .min_angle_relative_to_others(
                            id,
                            (controls[1].pos - controls[0].pos).normalize(),
                            &rails.transmute_lens::<(&Rail, &Spline)>().query(),
                        )
                } else {
                    90.
                };
                plan.status = if spline.curve_length() < RAIL_MIN_LENGTH
                    && plan.end_intersection_id.is_none()
                {
                    RailPlannerStatus::RailTooShort(spline.curve_length())
                } else if start_min_angle < RAIL_MIN_DELTA_RADIANS {
                    RailPlannerStatus::CurveTooShallow(start_min_angle)
                } else if plan.start_rail.is_some()
                    && plan.end_rail.is_some()
                    && plan.start_rail.unwrap() == plan.end_rail.unwrap()
                {
                    RailPlannerStatus::ExtendIntoSelf
                } else if plan.start_rail.is_some()
                    && trains
                        .iter()
                        .any(|train| train.rail == plan.start_rail.unwrap())
                {
                    RailPlannerStatus::TrainOnStartRail
                } else if plan.end_rail.is_some()
                    && trains
                        .iter()
                        .any(|train| train.rail == plan.end_rail.unwrap())
                {
                    RailPlannerStatus::TrainOnEndRail
                } else {
                    let points = spline.curve_points();
                    let first_segment = points[1] - points[0];
                    let max_angle = points
                        .iter()
                        .zip(points.iter().skip(1).zip(points.iter().skip(2)))
                        .fold(
                            controls[0].forward.angle_between(first_segment),
                            |max, (left, (middle, right))| {
                                let left = middle - left;
                                let right = right - middle;
                                let angle = left.angle_between(right);
                                let max = (angle).max(max);
                                max
                            },
                        );
                    if max_angle > RAIL_MAX_RADIANS {
                        RailPlannerStatus::CurveTooSharp(max_angle)
                    } else {
                        RailPlannerStatus::Valid
                    }
                };
            }

            // We have an intention to build
            if input.just_pressed(&PlayerBuildAction::Interact)
                && plan.status == RailPlannerStatus::Valid
            {
                spline.set_controls(controls);

                // Create the rail
                let mut ec = c.spawn_empty();
                let rail = Rail::new(&mut ec, &mut intersections, &plan, &spline, &asset);

                // Rail is inserted at the end because it moves rail, which is still used
                let mut modified_intersection_ids = vec![];
                let start_intersection_id = rail.joints[0].intersection_id;
                let end_intersection_id = rail.joints[1].intersection_id;
                modified_intersection_ids.extend([
                    rail.joints[0].intersection_id,
                    rail.joints[1].intersection_id,
                ]);
                ec.insert((rail, spline.clone()));

                // If required, split other rails
                if let Some(start) = plan.start_rail {
                    let (e, mut rail, mut spline) = rails.get_mut(start).unwrap();
                    rail.insert_intersection(
                        start_intersection_id,
                        e,
                        &controls[0].pos,
                        &mut spline,
                        &mut c,
                        &mut intersections,
                        &asset,
                        &mut modified_intersection_ids,
                        &mut ev_rail_removed,
                        &mut ev_intersection_removed,
                        None,
                    );
                }

                if let Some(end) = plan.end_rail {
                    let (e, mut rail, mut spline) = rails.get_mut(end).unwrap();
                    rail.insert_intersection(
                        end_intersection_id,
                        e,
                        &controls[1].pos,
                        &mut spline,
                        &mut c,
                        &mut intersections,
                        &asset,
                        &mut modified_intersection_ids,
                        &mut ev_rail_removed,
                        &mut ev_intersection_removed,
                        None,
                    );
                }

                modified_intersection_ids.sort();
                modified_intersection_ids.dedup();
                ev_changed.send_batch(
                    modified_intersection_ids
                        .iter()
                        .map(|x| RailIntersectionChangedEvent(*x)),
                );
                // Reset and update the plan
                controls[0].pos = controls[1].pos;
                controls[0].forward = -controls[1].forward;
                plan.start_intersection_id = Some(end_intersection_id);
                plan.end_intersection_id = None;
                plan.start_rail = None;
                plan.end_rail = None;
                cursor.manual_rotation = 0.;
            }
        }
        spline.set_controls(controls);
    });
}

fn update_rail_planner_feedback(
    mut feedback: ResMut<CursorFeedback>,
    mut plan: Query<(&RailPlanner, &mut PlaceablePreview)>,
    cursor: Query<&PlayerCursor>,
    input_entries: Res<AllInputContextEntries>,
) {
    let cursor = cursor.single();
    let mut feedback_entry = CursorFeedbackData::default();

    if !plan.is_empty() {
        let (plan, mut preview) = plan.single_mut();
        if plan.is_initial_placement() {
            feedback_entry.status = "Specifying initial orientation".into();
            preview.valid = true;
        } else {
            preview.valid = false;
            feedback_entry.error = match plan.status {
                RailPlannerStatus::Valid => {
                    preview.valid = true;
                    feedback_entry.error
                }
                RailPlannerStatus::CurveTooSharp(x) => format!(
                    "Curve Too Sharp {:.2} > {:.2}",
                    x.to_degrees(),
                    RAIL_MAX_RADIANS.to_degrees()
                )
                .into(),
                RailPlannerStatus::CurveTooShallow(x) => format!(
                    "Curve Too Shallow {:.2} < {:.2}",
                    x.to_degrees(),
                    RAIL_MIN_DELTA_RADIANS.to_degrees()
                )
                .into(),
                RailPlannerStatus::RailTooShort(x) => {
                    format!("Rail Too Short {:.2} < {:.2}", x, RAIL_MIN_LENGTH).into()
                }
                RailPlannerStatus::ExtendIntoSelf => {
                    "Can't expand and connect into same rail".into()
                }
                RailPlannerStatus::ExtendTooCloseToIntersection(x) => format!(
                    "Too close to intersection {:.2} < {:.2}",
                    x, RAIL_MIN_LENGTH
                )
                .into(),
                RailPlannerStatus::TrainOnStartRail => "Trains are on origin rail".into(),
                RailPlannerStatus::TrainOnEndRail => "Trains are on destination rail".into(),
            };

            let input = input_entries.get_input_entry(&PlayerBuildAction::CycleCurveMode);
            let input = if input.is_some() {
                &input.unwrap().input
            } else {
                ""
            };

            if plan.end_intersection_id.is_some() == true || plan.end_rail.is_some() {
                feedback_entry.status = "Fixed rotation to connect".into();
            } else {
                feedback_entry.status =
                    format!("<{}> CurveMode {}", input, cursor.curve_mode).into();
            }
        }
    }

    if feedback_entry.status.is_empty() == false || feedback_entry.error.is_empty() == false {
        feedback.entries.push(feedback_entry);
    }
}

fn draw_rail_planner(mut gizmos: Gizmos, q: Query<(&RailPlanner, &Spline)>) {
    q.into_iter().for_each(|(plan, spline)| {
        // Draw our control points
        gizmos.linestrip(
            spline.create_curve_control_points()[0],
            Color::srgb(0.5, 0.5, 0.5),
        );

        // Draw our curve
        let color = if plan.status == RailPlannerStatus::Valid {
            Color::srgb(0.1, 0.1, 1.0)
        } else {
            Color::srgb(1.0, 0.1, 0.1)
        };
        gizmos.linestrip(spline.curve_points_projected().clone(), color);
        // info!(
        //     "points {:?}\n
        //     pos {:?}",
        //     points,
        //     curve.iter_positions(STEPS).collect::<Vec<Vec3>>()
        // );

        gizmos.line(
            spline.controls()[0].pos,
            spline.controls()[1].pos,
            Color::srgb(0.7, 0.7, 0.7),
        );
        gizmos.line(
            spline.controls()[0].pos,
            spline.controls()[0].pos + spline.controls()[0].forward.as_vec3(),
            Color::srgb(0.7, 0.0, 0.0),
        );
        gizmos.line(
            spline.controls()[1].pos,
            spline.controls()[1].pos + spline.controls()[1].forward.as_vec3(),
            Color::srgb(0.0, 0.0, 0.7),
        );
    });
}
