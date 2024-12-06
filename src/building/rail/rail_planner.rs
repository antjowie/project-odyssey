use bevy::gizmos;

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
    pub end: Vec3,
    pub target_joint: Option<RailEntityJointPair>,
}

impl RailPlanner {
    fn new(start_pos: Vec3) -> Self {
        RailPlanner {
            start: start_pos,
            end: start_pos,
            target_joint: None,
        }
    }
}

// We store target_joint info in a specific struct since we can't reference other RailPathJoint
pub struct RailEntityJointPair {
    rail_entity: Entity,
    is_start_joint: bool,
}

fn create_rail_planner(
    mut c: Commands,
    q: Query<Entity, (With<RailPlanner>, With<NetOwner>)>,
    player_state: Query<(&PlayerCursor, &ActionState<PlayerInput>), With<NetOwner>>,
    rail_states: Query<(Entity, &RailPathState)>,
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

        plan.target_joint = rail_states.into_iter().find_map(|(e, state)| {
            get_joint_collision(state, cursor_sphere).and_then(|joint| {
                Some(RailEntityJointPair {
                    rail_entity: e,
                    is_start_joint: state.start_joint.pos == joint.pos,
                })
            })
        });

        c.spawn((RailPlanner::new(cursor.build_pos), NetOwner));
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
    mut c: Commands,
    mut q: Query<&mut RailPlanner>,
    player_state: Query<(&PlayerCursor, &ActionState<PlayerInput>), With<NetOwner>>,
) {
    let (cursor, input) = player_state.single();

    q.iter_mut().for_each(|mut plan| {
        plan.end = cursor.build_pos;

        // Update state
        if input.just_pressed(&PlayerInput::Interact) {
            c.spawn(RailBundle::new(RailPathState::new(&plan)));
            plan.start = plan.end;
        }
    });
}

fn draw_rail_planner(mut gizmos: Gizmos, q: Query<&RailPlanner>) {
    q.into_iter().for_each(|plan| {
        gizmos.line(plan.start, plan.end, Color::WHITE);
    });
}
