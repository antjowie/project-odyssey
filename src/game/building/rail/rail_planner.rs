use std::f32::consts::FRAC_PI_2;

use bevy::math::vec3;

/// Logic responsible for generating a preview of what RailBuilding will be built
use super::*;

pub fn rail_planner_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            create_rail_planner,
            update_rail_planner,
            update_rail_planner_status,
            draw_rail_planner,
            preview_initial_rail_planner_placement.run_if(not(any_with_component::<RailPlanner>)),
        )
            .run_if(any_with_component::<InputContext<PlayerBuildAction>>),
    );
}

#[derive(Component)]
#[require(Text, TextFont(|| default_text_font()))]
pub struct RailPlanner {
    pub start: Vec3,
    /// The direction in which start joint should expand to create joint, points
    /// towards the curve
    pub start_forward: Vec3,
    pub start_intersection_id: Option<u32>,
    /// Initial placement is used together with the presence of a start joint
    /// If we have no initial placement, we want the first placement to confirm
    /// the start_forward orientation
    pub is_initial_placement: bool,
    pub end: Vec3,
    pub end_forward: Vec3,
    pub end_intersection_id: Option<u32>,
    pub status: RailPlannerStatus,
}

impl RailPlanner {
    fn new(start_pos: Vec3) -> Self {
        RailPlanner {
            start: start_pos,
            start_forward: start_pos.normalize_or(Vec3::X),
            start_intersection_id: None,
            is_initial_placement: true,
            end: start_pos,
            end_forward: start_pos.normalize_or(Vec3::X),
            end_intersection_id: None,
            status: RailPlannerStatus::Valid,
        }
    }

    fn is_initial_placement(&self) -> bool {
        self.is_initial_placement && self.start_intersection_id.is_none()
    }
}

#[derive(Default, PartialEq, Debug)]
pub enum RailPlannerStatus {
    #[default]
    Valid,
    CurveTooSharp(f32),
    // Our delta angle is too close to any other curves in our joint
    CurveTooShallow(f32),
    RailTooShort(f32),
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
        c.entity(trigger.entity()).despawn();
    }
}

fn create_rail_planner(
    mut c: Commands,
    q: Query<Entity, With<RailPlanner>>,
    player_state: Query<(Entity, &PlayerCursor, &ActionState<PlayerBuildAction>)>,
    intersections: Res<RailIntersections>,
    mut event: EventReader<PlayerStateEvent>,
) {
    // Hacky, but we want to ignore placing this on the switch to view mode
    for ev in event.read() {
        if ev.new_state == PlayerState::Building && ev.old_state == PlayerState::Viewing {
            return;
        }
    }

    // TODO: Creation of correct preview visual should be handled generically
    //       if we want to build more then only rails which we want. We still
    //       need to place trains on the rails
    let (state_e, cursor, input) = player_state.single();
    if q.is_empty() && input.just_pressed(&PlayerBuildAction::Interact) {
        let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

        let mut plan = RailPlanner::new(cursor.build_pos);

        plan.start_intersection_id = intersections
            .get_intersect_collision(&cursor_sphere)
            .and_then(|x| {
                plan.start = x.1.collision.center.into();
                plan.start_forward = x.1.get_nearest_forward(cursor.world_pos);

                Some(*x.0)
            });

        c.spawn(plan)
            .insert(BuildingPreview::new(state_e))
            .observe(handle_build_state_cancel_event);
    }
}

fn preview_initial_rail_planner_placement(
    mut gizmos: Gizmos,
    cursor: Single<&PlayerCursor>,
    intersections: Res<RailIntersections>,
) {
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    gizmos.cuboid(
        Transform::from_translation(cursor.build_pos).with_scale(Vec3::splat(2.0)),
        Color::WHITE,
    );

    if let Some(x) = intersections.get_intersect_collision(&cursor_sphere) {
        let start: Vec3 = x.1.collision.center.into();
        let end = start + x.1.get_nearest_forward(cursor.world_pos) * 10.;

        gizmos
            .arrow(start, end, Color::srgb(1., 1., 0.))
            .with_tip_length(5.);
    }
}

fn update_rail_planner(
    mut gizmos: Gizmos,
    mut c: Commands,
    mut q: Query<&mut RailPlanner>,
    rails: Query<&Rail>,
    mut player_states: Query<(&mut PlayerCursor, &ActionState<PlayerBuildAction>)>,
    mut intersections: ResMut<RailIntersections>,
) {
    let (mut cursor, input) = player_states.single_mut();

    q.iter_mut().for_each(|mut plan| {
        plan.end = cursor.build_pos;

        let delta = plan.end - plan.start;
        let towards = delta.normalize();

        // This is our initial placement, we don't have an orientation yet
        if plan.is_initial_placement() {
            plan.start_forward = towards;
            plan.end_forward = -towards;

            if input.just_pressed(&PlayerBuildAction::Interact) {
                plan.is_initial_placement = false;
            }
        } else {
            // Calculate forwards which control curve shape
            match cursor.curve_mode {
                PathCurveMode::Curve => {
                    let incidence = plan.start_forward;
                    let towards_2d = Vec2::from_angle(FRAC_PI_2).rotate(towards.xz());
                    let normal = vec3(towards_2d.x, 0., towards_2d.y);
                    gizmos.line(plan.start, plan.start + normal, Color::BLACK);
                    plan.end_forward = -incidence.reflect(normal);
                }
                PathCurveMode::Straight => {
                    plan.end_forward = -plan.start_forward;
                }
                PathCurveMode::Chase => {
                    plan.end_forward = -towards;
                }
            }
            plan.end_forward = Quat::from_rotation_y(cursor.manual_rotation) * plan.end_forward;

            // Check if we hover over another rail, if so we align our end_forward
            let sphere = BoundingSphere::new(cursor.world_pos, 0.1);
            if let Some(x) = intersections.get_intersect_collision(&sphere) {
                plan.end = x.1.collision.center.into();
                plan.end_forward = x.1.get_nearest_forward(cursor.world_pos);
                plan.end_intersection_id = Some(*x.0);
            } else {
                plan.end_intersection_id = None;
            }

            // Validate our plan
            let length = delta.length();
            let start_min_angle = if let Some(id) = plan.start_intersection_id {
                intersections
                    .intersections
                    .get(&id)
                    .unwrap()
                    .min_angle_relative_to_others(id, (plan.end - plan.start).normalize(), &rails)
            } else {
                90.
            };
            plan.status = if length < RAIL_MIN_LENGTH && plan.end_intersection_id.is_none() {
                RailPlannerStatus::RailTooShort(delta.length())
            } else if start_min_angle < RAIL_MIN_DELTA_RADIANS {
                RailPlannerStatus::CurveTooShallow(start_min_angle)
            } else {
                let points: Vec<Vec3> = create_curve_points(create_curve_control_points(
                    plan.start,
                    plan.start_forward,
                    plan.end,
                    plan.end_forward,
                ));
                let first_segment = points[1] - points[0];
                let max_angle = points
                    .iter()
                    .zip(points.iter().skip(1).zip(points.iter().skip(2)))
                    .fold(
                        plan.start_forward.angle_between(first_segment),
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

            // We have an intention to build
            if input.just_pressed(&PlayerBuildAction::Interact)
                && plan.status == RailPlannerStatus::Valid
            {
                let mut ec = c.spawn_empty();
                let rail = Rail::new(ec.id(), &mut intersections, &plan);
                plan.start = plan.end;
                plan.start_forward = -plan.end_forward;
                plan.start_intersection_id = Some(rail.joints[RAIL_END_JOINT].intersection_id);
                ec.insert(rail);
                cursor.manual_rotation = 0.;
            }
        }
    });
}

fn update_rail_planner_status(
    mut q: Query<(&RailPlanner, &mut Text, &mut Node)>,
    cursor: Query<&PlayerCursor>,
    input_entries: Res<AllInputContextEntries>,
) {
    let cursor = cursor.single();
    q.iter_mut().for_each(|(plan, mut text, mut node)| {
        if plan.is_initial_placement() {
            text.0 = "Specifying initial orientation".into();
        } else {
            text.0 = match plan.status {
                RailPlannerStatus::Valid => "".into(),
                RailPlannerStatus::CurveTooSharp(x) => format!(
                    "Curve Too Sharp {:.2} > {:.2}",
                    x.to_degrees(),
                    RAIL_MAX_RADIANS.to_degrees()
                )
                .into(),
                RailPlannerStatus::CurveTooShallow(x) => {
                    // "Rail angle too close to neighbors".into()
                    format!(
                        "Curve Too Shallow {:.2} < {:.2}",
                        x.to_degrees(),
                        RAIL_MIN_DELTA_RADIANS.to_degrees()
                    )
                    .into()
                }
                RailPlannerStatus::RailTooShort(x) => {
                    format!("Rail Too Short {:.2} < {:.2}", x, RAIL_MIN_LENGTH).into()
                }
            };

            let input = input_entries.get_input_entry(&PlayerBuildAction::CycleCurveMode);
            let input = if input.is_some() {
                &input.unwrap().input
            } else {
                ""
            };

            text.0 += format!("\n<{}> CurveMode {}", input, cursor.curve_mode).as_str();
        }

        if let Some(pos) = cursor.screen_pos {
            node.left = Val::Px(pos.x);
            node.top = Val::Px(pos.y - 48.);
        }
    });
}

fn draw_rail_planner(mut gizmos: Gizmos, q: Query<&RailPlanner>) {
    q.into_iter().for_each(|plan| {
        let points =
            create_curve_control_points(plan.start, plan.start_forward, plan.end, plan.end_forward);
        // Draw our control points
        gizmos.linestrip(points[0], Color::srgb(0.5, 0.5, 0.5));

        // Draw our curve
        let color = if plan.status == RailPlannerStatus::Valid {
            Color::srgb(0.1, 0.1, 1.0)
        } else {
            Color::srgb(1.0, 0.1, 0.1)
        };
        gizmos.linestrip(create_curve_points(points), color);
        // info!(
        //     "points {:?}\n
        //     pos {:?}",
        //     points,
        //     curve.iter_positions(STEPS).collect::<Vec<Vec3>>()
        // );

        gizmos.line(plan.start, plan.end, Color::srgb(0.7, 0.7, 0.7));
        gizmos.line(
            plan.start,
            plan.start + plan.start_forward,
            Color::srgb(0.7, 0.0, 0.0),
        );
        gizmos.line(
            plan.end,
            plan.end + plan.end_forward,
            Color::srgb(0.0, 0.0, 0.7),
        );
    });
}
