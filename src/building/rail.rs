use bevy::{
    math::bounding::{Aabb3d, BoundingSphere, BoundingVolume, IntersectsVolume},
    prelude::*,
};
use leafwing_input_manager::prelude::*;

use crate::building::*;
use crate::game::*;

pub fn add_rail_systems(app: &mut App) {
    // app.add_systems(Update, (on_place_rail, debug_draw_rail_path));
    app.add_systems(
        Update,
        (
            (create_rail_planner, update_rail_planner, draw_rail_planner)
                .run_if(in_player_state(PlayerState::Building)),
            destroy_rail_planner,
            debug_draw_rail_path,
        ),
    );
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

/// Contains the details to build and connect a rail
#[derive(Component)]
pub struct RailPathState {
    pub start_joint: RailPathJoint,
    pub end_joint: RailPathJoint,
}

impl RailPathState {
    fn new(plan: &RailPlanner) -> RailPathState {
        let start = plan.start;
        let end = plan.end;
        let size = ((end - start).length() * 0.5).min(1.0);

        // TODO: Actually update joint bindings
        RailPathState {
            start_joint: RailPathJoint {
                pos: start,
                collision: Aabb3d::new(start, Vec3::splat(size)),
                left: None,
                straight: None,
                right: None,
            },
            end_joint: RailPathJoint {
                pos: end,
                collision: Aabb3d::new(end, Vec3::splat(size)),
                left: None,
                straight: None,
                right: None,
            },
        }
    }
}

pub struct RailPathJoint {
    pub pos: Vec3,
    pub collision: Aabb3d,
    pub left: Option<Entity>,
    pub straight: Option<Entity>,
    pub right: Option<Entity>,
}

impl Default for RailPathJoint {
    fn default() -> Self {
        RailPathJoint {
            collision: Aabb3d::new(Vec3::ZERO, Vec3::ZERO),
            ..default()
        }
    }
}

#[derive(Bundle)]
pub struct RailBundle {
    pub pbr: PbrBundle,
    pub building: Building,
    pub state: RailPathState,
}

impl RailBundle {
    fn new(state: RailPathState) -> RailBundle {
        RailBundle {
            pbr: PbrBundle::default(),
            building: Building::default(),
            state: state,
        }
    }
}

#[derive(Default)]
pub struct SpawnRail {
    pub is_preview: bool,
    pub transform: Transform,
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

pub fn debug_draw_rail_path(
    mut gizmos: Gizmos,
    q: Query<&RailPathState>,
    cursor: Query<&PlayerCursor, With<NetOwner>>,
) {
    let cursor = cursor.single();
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    q.into_iter().for_each(|state| {
        gizmos.line(state.start_joint.pos, state.end_joint.pos, Color::WHITE);
        let collision = get_joint_collision(state, cursor_sphere);
        gizmos.cuboid(
            Transform::from_translation(state.start_joint.collision.center().into())
                .with_scale(state.start_joint.collision.half_size().into()),
            if collision.is_some_and(|joint| joint.pos == state.start_joint.pos) {
                Color::srgb(1.0, 0.0, 0.0)
            } else {
                Color::WHITE
            },
        );
        gizmos.cuboid(
            Transform::from_translation(state.end_joint.collision.center().into())
                .with_scale(state.end_joint.collision.half_size().into()),
            if collision.is_some_and(|joint| joint.pos == state.end_joint.pos) {
                Color::srgb(1.0, 0.0, 0.0)
            } else {
                Color::WHITE
            },
        );
    });
}

fn get_joint_collision(
    rail_path: &RailPathState,
    sphere: BoundingSphere,
) -> Option<&RailPathJoint> {
    if rail_path.start_joint.collision.intersects(&sphere) {
        Some(&rail_path.start_joint)
    } else if rail_path.end_joint.collision.intersects(&sphere) {
        Some(&rail_path.end_joint)
    } else {
        None
    }
    // let distance = (rail_path.end_joint.pos - rail_path.start_joint.pos).length();
    // let start = Aabb3d::new(rail_path.start_joint.pos, Vec3::splat(distance * 0.5));
    // let end = Aabb3d::new(rail_path.end_joint.pos, Vec3::splat(distance * 0.5));
}

pub fn create_rail_planner(
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

pub fn destroy_rail_planner(
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

pub fn update_rail_planner(
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

pub fn draw_rail_planner(mut gizmos: Gizmos, q: Query<&RailPlanner>) {
    q.into_iter().for_each(|plan| {
        gizmos.line(plan.start, plan.end, Color::WHITE);
    });
}

fn on_remove_build_preview_component(
    mut c: Commands,
    mut q: Query<(&mut Handle<StandardMaterial>, &BuildingPreview), With<Building>>,
    mut removed: RemovedComponents<BuildingPreview>,
) {
    for entity in removed.read() {
        if let Ok((mut handle, preview)) = q.get_mut(entity) {
            c.entity(entity).remove::<NotShadowCaster>();
            *handle = preview.orig_material.clone();
        }
    }
}

pub fn on_place_rail(
    mut c: Commands,
    mut q: Query<
        (Entity, &Transform, &mut RailPathState, &BuildingPreview),
        Changed<BuildingPreview>,
    >,
    cursor: Query<&PlayerCursor, With<NetOwner>>,
    mut has_placed_start: Local<bool>,
) {
    let cursor = cursor.single();
    let cursor_sphere = BoundingSphere::new(cursor.build_pos, 0.1);

    q.iter_mut().for_each(|(e, t, mut state, preview)| {
        if !preview.wants_to_place {
            return;
        }

        // c.add(SpawnRail {
        //     transform: t.clone(),
        //     ..default()
        // });
    });
}
