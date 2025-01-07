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
#[require(Text, BuildingPreview, TextFont(|| default_text_font()))]
pub struct RailPlanner {
    pub start: Vec3,
    pub start_forward: Vec3,
    pub end: Vec3,
    pub end_forward: Vec3,
    // Joint we expand from
    pub start_joint: Option<RailPathJointRef>,
    // Joint we end with, and want to connect to
    pub end_joint: Option<RailPathJointRef>,
    pub status: RailPlannerStatus,
}

impl RailPlanner {
    fn new(start_pos: Vec3) -> Self {
        RailPlanner {
            start: start_pos,
            start_forward: start_pos.normalize_or(Vec3::X),
            end: start_pos,
            end_forward: start_pos.normalize_or(Vec3::X),
            start_joint: None,
            end_joint: None,
            status: RailPlannerStatus::Valid,
        }
    }
}

#[derive(Default, PartialEq)]
pub enum RailPlannerStatus {
    #[default]
    Valid,
    CurveTooSharp(f32),
    // Our delta angle is too close to any other curves in our joint
    CurveTooShallow(f32),
    RailTooShort(f32),
}

#[derive(Default, Reflect, PartialEq, Debug, DisplayDebug)]
pub enum PathRotationMode {
    #[default]
    // Keep aligned with start joint
    Straight,
    // Share same angle between start and end joint
    Curve,
    // Align end joint with direction between end and start point
    Chase,
}

fn create_rail_planner(
    mut c: Commands,
    q: Query<Entity, With<RailPlanner>>,
    player_state: Query<(&PlayerCursor, &ActionState<PlayerBuildAction>)>,
    rail_states: Query<(Entity, &Rail)>,
    mut event: EventReader<PlayerStateEvent>,
) {
    // Hacky, but we want to ignore placing this on the switch to view mode
    for e in event.read() {
        if e.new_state == PlayerState::Building && e.old_state == PlayerState::Viewing {
            return;
        }
    }

    // TODO: Creation of correct preview visual should be handled generically if we want to build more then only rails
    //       which we want. We still need to place trains on the rails
    let (cursor, input) = player_state.single();
    if q.is_empty() && input.just_pressed(&PlayerBuildAction::Interact) {
        let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

        let mut plan = RailPlanner::new(cursor.build_pos);

        plan.start_joint = rail_states.into_iter().find_map(|(e, state)| {
            get_joint_collision(state, cursor_sphere).and_then(|joint| {
                plan.start = joint.pos;
                plan.start_forward = -joint.forward;
                Some(RailPathJointRef {
                    rail_entity: e,
                    joint_idx: if state.joints[RAIL_START_JOINT].pos == joint.pos {
                        RAIL_START_JOINT
                    } else {
                        RAIL_END_JOINT
                    },
                })
            })
        });

        c.spawn(plan);
    }
}

fn preview_initial_rail_planner_placement(mut gizmos: Gizmos, cursor: Query<&PlayerCursor>) {
    let cursor = cursor.single();

    gizmos.cuboid(
        Transform::from_translation(cursor.build_pos).with_scale(Vec3::splat(2.0)),
        Color::WHITE,
    );
}

fn update_rail_planner(
    mut gizmos: Gizmos,
    mut c: Commands,
    mut q: Query<&mut RailPlanner>,
    mut rail_states: Query<(Entity, &mut Rail)>,
    player_state: Query<(&PlayerCursor, &ActionState<PlayerBuildAction>)>,
) {
    let (cursor, input) = player_state.single();
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    q.iter_mut().for_each(|(mut plan)| {
        plan.end = cursor.build_pos;
        // Min length check
        if plan.end.distance_squared(plan.start) < 1.0 {
            plan.end = (plan.end - plan.start).normalize() + plan.start;
        }

        let delta = plan.end - plan.start;
        let towards = delta.normalize();
        match cursor.rotation_mode {
            PathRotationMode::Straight => {
                if plan.start_joint.is_none() {
                    plan.start_forward = -towards;
                    plan.end_forward = towards;
                } else {
                    // plan.end_forward = plan.start_forward.reflect(towards.any_orthonormal_vector());
                    plan.end_forward = -plan.start_forward;
                }
            }
            PathRotationMode::Curve => {
                let incidence = -plan.start_forward;
                let towards_2d = Vec2::from_angle(FRAC_PI_2).rotate(towards.xz());
                let normal = vec3(towards_2d.x, 0., towards_2d.y);
                gizmos.line(plan.start, plan.start + normal, Color::BLACK);
                plan.end_forward = incidence.reflect(normal);
            }
            PathRotationMode::Chase => {
                plan.end_forward = towards;
            }
        }
        plan.end_forward = Quat::from_rotation_y(cursor.manual_rotation) * plan.end_forward;

        // Check if we hover over a joint for end pos
        plan.end_joint = rail_states.into_iter().find_map(|(e, state)| {
            get_joint_collision(state, cursor_sphere).and_then(|joint| {
                plan.end = joint.pos;
                plan.end_forward = -joint.forward;
                Some(RailPathJointRef {
                    rail_entity: e,
                    joint_idx: if state.joints[RAIL_START_JOINT].pos == joint.pos {
                        RAIL_START_JOINT
                    } else {
                        RAIL_END_JOINT
                    },
                })
            })
        });

        // Validate our plan
        let length = delta.length();

        plan.status = if length < RAIL_MIN_LENGTH && plan.end_joint.is_none() {
            RailPlannerStatus::RailTooShort(delta.length())
            // TODO: Check joints
        } else if false {
            RailPlannerStatus::CurveTooShallow(0.)
        } else {
            let points: Vec<Vec3> = create_curve_points(create_curve_control_points(
                plan.start,
                plan.start_forward,
                plan.end,
                plan.end_forward,
            ));
            let first_segment = points[1] - points[0];
            let angle = points
                .iter()
                .zip(points.iter().skip(1).zip(points.iter().skip(2)))
                .fold(
                    (-plan.start_forward).angle_between(first_segment),
                    |max, (left, (middle, right))| {
                        let left = middle - left;
                        let right = right - middle;
                        let angle = left.angle_between(right);
                        let max = (angle).max(max);
                        max
                    },
                );
            if angle > RAIL_MAX_RADIANS {
                RailPlannerStatus::CurveTooSharp(angle)
            } else {
                RailPlannerStatus::Valid
            }
        };

        // We have an intention to build
        if input.just_pressed(&PlayerBuildAction::Interact)
            && plan.status == RailPlannerStatus::Valid
        {
            let mut rail = c.spawn_empty();
            rail.insert(Rail::new(
                rail.id(),
                &mut rail_states.transmute_lens::<&mut Rail>().query(),
                &plan,
            ));
            plan.start = plan.end;
            plan.start_forward = -plan.end_forward;
            plan.start_joint = Some(RailPathJointRef {
                rail_entity: rail.id(),
                joint_idx: RAIL_END_JOINT,
            });
        }
    });
}

fn update_rail_planner_status(
    mut q: Query<(&RailPlanner, &mut Text, &mut Node)>,
    cursor: Query<&PlayerCursor>,
) {
    let cursor = cursor.single();
    q.iter_mut().for_each(|(plan, mut text, mut node)| {
        text.0 = match plan.status {
            RailPlannerStatus::Valid => "".into(),
            RailPlannerStatus::CurveTooSharp(x) => {
                format!("Curve Too Sharp {:.2}", x.to_degrees()).into()
            }
            RailPlannerStatus::CurveTooShallow(x) => {
                format!("Curve Too Shallow {:.2}", x.to_degrees()).into()
            }
            RailPlannerStatus::RailTooShort(x) => {
                format!("Rail Too Short {:.2} < {:2}", x, RAIL_MIN_LENGTH).into()
            }
        };

        text.0 += format!("\nCurveMode {}", cursor.rotation_mode).as_str();

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
