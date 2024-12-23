use std::f32::consts::FRAC_PI_2;

use bevy::{gizmos, math::vec3};

/// Logic responsible for generating a preview of what RailBuilding will be built
use super::*;

pub fn rail_planner_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            (
                create_rail_planner,
                update_rail_planner,
                draw_rail_planner,
                preview_initial_rail_planner_placement
                    .run_if(not(any_with_component::<RailPlanner>)),
            )
                .run_if(in_player_state(PlayerState::Building)),
            destroy_rail_planner,
        ),
    );
}

#[derive(Component)]
pub struct RailPlanner {
    pub start: Vec3,
    pub start_forward: Vec3,
    pub end: Vec3,
    pub end_forward: Vec3,
    // Joint we expand from
    pub start_joint: Option<RailPathJointRef>,
    // Joint we end with, and want to connect to
    pub end_joint: Option<RailPathJointRef>,
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
        }
    }
}

fn create_rail_planner(
    mut c: Commands,
    q: Query<Entity, (With<RailPlanner>, With<NetOwner>)>,
    player_state: Query<(&PlayerCursor, &ActionState<PlayerInput>), With<NetOwner>>,
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
    if q.is_empty() && input.just_pressed(&PlayerInput::Interact) {
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

        c.spawn((plan, NetOwner));
    }
}

fn destroy_rail_planner(
    mut c: Commands,
    q: Query<Entity, (With<RailPlanner>, With<NetOwner>)>,
    mut event: EventReader<PlayerStateEvent>,
) {
    for e in event.read() {
        if e.new_state == PlayerState::Viewing && e.old_state == PlayerState::Building {
            q.into_iter().for_each(|e| {
                c.entity(e).despawn();
            });
        }
    }
}

fn preview_initial_rail_planner_placement(
    mut gizmos: Gizmos,
    cursor: Query<&PlayerCursor, With<NetOwner>>,
) {
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
    player_state: Query<(&PlayerCursor, &ActionState<PlayerInput>), With<NetOwner>>,
) {
    let (cursor, input) = player_state.single();
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    q.iter_mut().for_each(|mut plan| {
        plan.end = cursor.build_pos;
        // Min length check
        if plan.end.distance_squared(plan.start) < 1.0 {
            plan.end = (plan.end - plan.start).normalize() + plan.start;
        }

        let towards = (plan.end - plan.start).normalize();
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

        // Check if we connected with an joint for our end
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

        if input.just_pressed(&PlayerInput::Interact) {
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

fn draw_rail_planner(mut gizmos: Gizmos, q: Query<&RailPlanner>) {
    q.into_iter().for_each(|plan| {
        // Draw line
        let length = (plan.end - plan.start).length();
        // let seg_distance = plan.start.distance(plan.end) * 0.25;

        let points = [[
            plan.start,
            plan.start - plan.start_forward * length * 0.5,
            plan.end - plan.end_forward * length * 0.5,
            plan.end,
        ]];

        gizmos.linestrip(points[0], Color::srgb(1.0, 0.1, 0.1));

        let curve = CubicBezier::new(points).to_curve().unwrap();
        const STEPS: usize = 10;
        gizmos.linestrip(curve.iter_positions(STEPS), Color::WHITE);
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
