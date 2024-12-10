use std::f32::consts::PI;

use bevy::color::palettes::tailwind::*;
use bevy::picking::pointer::PointerInteraction;
use bevy::{math::*, prelude::*, window::PrimaryWindow};
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;

use crate::building::*;
use crate::camera::*;

/// All game systems and rules
/// 100 units is 1 meter
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerInput>::default());
        app.add_event::<PlayerStateEvent>();
        app.add_systems(PreUpdate, update_cursor.after(InputManagerSystem::Update));
        app.add_systems(
            Update,
            (
                process_state_change,
                // draw_mesh_intersections,
                draw_build_grid.run_if(in_player_state(PlayerState::Building)),
                // snap_building_preview_to_build_pos,
                // validate_building_preview.run_if(on_timer(Duration::from_secs(1))),
                // process_view_state_input.run_if(in_player_state(PlayerState::Viewing)),
                // process_state_change,
                // create_building_preview,
            ),
        );
        app.register_type::<PlayerCursor>();
    }
}

#[derive(Component)]
pub struct NetOwner;

#[derive(Bundle, Default)]
pub struct PlayerStateBundle {
    pub state: PlayerState,
    pub cursor: PlayerCursor,
    pub input: InputManagerBundle<PlayerInput>,
}

#[derive(Component, Default, PartialEq, Clone)]
pub enum PlayerState {
    #[default]
    Viewing,
    Building,
}

pub fn in_player_state(
    state: PlayerState,
) -> impl FnMut(Query<&PlayerState, With<NetOwner>>) -> bool {
    move |query: Query<&PlayerState, With<NetOwner>>| !query.is_empty() && *query.single() == state
}

#[derive(Default, Reflect, PartialEq)]
pub enum PathRotationMode {
    // Use manually defined rotation
    Manual,
    // Automatically rotate to an ideal position
    #[default]
    Straight,
}

#[derive(Event)]
pub struct PlayerStateEvent {
    pub new_state: PlayerState,
    pub old_state: PlayerState,
}

/// Component that tracks the cursor position
#[derive(Component, Default, Reflect)]
pub struct PlayerCursor {
    pub screen_pos: Option<Vec2>,
    pub should_snap_to_grid: bool,
    // Cached build rotation
    pub manual_rotation: f32,
    pub rotation_mode: PathRotationMode,
    // Can be world or grid pos based on user desire
    pub build_pos: Vec3,
    pub world_pos: Vec3,
    pub prev_world_pos: Vec3,
    pub world_grid_pos: Vec3,
}

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerInput {
    Interact,
    Cancel,
    Pause,
    SnapToGrid,
    Rotate,
    CounterRotate,
    SnapRotate,
    SnapCounterRotate,
    CyclePathRotateMode,
}

impl PlayerInput {
    pub fn default_player_mapping() -> InputMap<PlayerInput> {
        InputMap::default()
            .with(PlayerInput::Interact, MouseButton::Left)
            .with(PlayerInput::Cancel, KeyCode::KeyE)
            .with(PlayerInput::Cancel, KeyCode::Escape)
            .with(PlayerInput::Pause, KeyCode::Escape)
            .with(PlayerInput::SnapToGrid, KeyCode::ControlLeft)
            .with(PlayerInput::Rotate, KeyCode::KeyR)
            .with(
                PlayerInput::SnapRotate,
                ButtonlikeChord::modified(ModifierKey::Control, KeyCode::KeyR),
            )
            .with(
                PlayerInput::CounterRotate,
                ButtonlikeChord::modified(ModifierKey::Shift, KeyCode::KeyR),
            )
            .with(
                PlayerInput::SnapCounterRotate,
                ButtonlikeChord::modified(ModifierKey::Shift, KeyCode::KeyR)
                    .with(ModifierKey::Control),
            )
            .with(PlayerInput::CyclePathRotateMode, KeyCode::Tab)
    }
}

fn update_cursor(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&PanOrbitCamera, &Camera, &GlobalTransform)>,
    mut q: Query<(&mut PlayerCursor, &ActionState<PlayerInput>), With<NetOwner>>,
    time: Res<Time>,
) {
    let window = windows.single();
    let (pan_cam, camera, global_transform) = cameras.iter().find(|(_, c, _)| c.is_active).unwrap();
    let (mut cursor, input) = q.single_mut();

    // Check if cursor is in window
    cursor.prev_world_pos = cursor.world_pos;
    cursor.screen_pos = window.cursor_position();
    if let Some(ray) = cursor
        .screen_pos
        .and_then(|pos| camera.viewport_to_world(global_transform, pos).ok())
    {
        // Check if cursor intersects ground
        if let Some(len) = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y)) {
            cursor.world_pos = ray.origin + ray.direction * len;
            // gizmos.sphere(cursor.position, Quat::IDENTITY, 10.0, RED);
        }
    } else {
        // Set these values to camera center, in case we do gamepad implementation
        cursor.world_pos = pan_cam.center;
    }
    cursor.world_grid_pos = cursor.world_pos.round();

    if input.just_pressed(&PlayerInput::SnapToGrid) {
        cursor.should_snap_to_grid = !cursor.should_snap_to_grid;
    }

    if input.pressed(&PlayerInput::Rotate) {
        cursor.manual_rotation -= PI * 0.5 * time.delta_secs();
    }
    if input.pressed(&PlayerInput::CounterRotate) {
        cursor.manual_rotation += PI * 0.5 * time.delta_secs();
    }

    const SNAP_ROT: f32 = PI * 0.5;
    if input.just_pressed(&PlayerInput::SnapRotate) {
        cursor.manual_rotation = (cursor.manual_rotation / SNAP_ROT).round() * SNAP_ROT - SNAP_ROT;
    }
    if input.just_pressed(&PlayerInput::SnapCounterRotate) {
        cursor.manual_rotation = (cursor.manual_rotation / SNAP_ROT).round() * SNAP_ROT + SNAP_ROT;
    }

    if input.just_pressed(&PlayerInput::CyclePathRotateMode) {
        cursor.rotation_mode = if cursor.rotation_mode == PathRotationMode::Straight {
            PathRotationMode::Manual
        } else {
            PathRotationMode::Straight
        }
    }

    cursor.build_pos = if cursor.should_snap_to_grid {
        cursor.world_grid_pos
    } else {
        cursor.world_pos
    };
}

fn process_state_change(
    mut q: Query<(&mut PlayerState, &ActionState<PlayerInput>), With<NetOwner>>,
    mut previews: Query<&mut BuildingPreview, With<NetOwner>>,
    mut ev_player_state: EventWriter<PlayerStateEvent>,
    mut exit: EventWriter<AppExit>,
) {
    let (mut state, input) = q.single_mut();
    let old_state = state.clone();

    match *state {
        PlayerState::Viewing => {
            if input.just_pressed(&PlayerInput::Interact) {
                *state = PlayerState::Building;
            }

            if input.just_pressed(&PlayerInput::Pause) {
                exit.send(AppExit::Success);
            }
        }
        PlayerState::Building => {
            let wants_to_place = input.just_pressed(&PlayerInput::Interact);
            previews.iter_mut().for_each(|mut preview| {
                preview.wants_to_place = wants_to_place;
            });

            if input.just_pressed(&PlayerInput::Cancel) {
                *state = PlayerState::Viewing;
            }
        }
    }

    if *state != old_state {
        ev_player_state.send(PlayerStateEvent {
            old_state: old_state,
            new_state: state.clone(),
        });
    }
}

fn draw_mesh_intersections(pointers: Query<&PointerInteraction>, mut gizmos: Gizmos) {
    for (point, normal) in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position.zip(hit.normal))
    {
        gizmos.sphere(point, 0.05, RED_500);
        gizmos.arrow(point, point + normal.normalize() * 0.5, PINK_100);
    }
}

fn create_building_preview(
    q: Query<Entity, (With<NetOwner>, With<BuildingPreview>)>,
    mut c: Commands,
    mut event: EventReader<PlayerStateEvent>,
) {
    for e in event.read() {
        if e.new_state == PlayerState::Building && e.old_state == PlayerState::Viewing {
            // c.add(SpawnRail {
            //     is_preview: true,
            //     ..default()
            // });
        } else if e.new_state == PlayerState::Viewing && e.old_state == PlayerState::Building {
            q.into_iter().for_each(|e| {
                c.entity(e).despawn();
            });
        }
    }
}

fn snap_building_preview_to_build_pos(
    mut q: Query<&mut Transform, (With<NetOwner>, With<BuildingPreview>)>,
    cursor: Query<&PlayerCursor, With<NetOwner>>,
) {
    let cursor = cursor.single();

    q.iter_mut().for_each(|mut transform| {
        transform.translation = cursor.build_pos;
    });
}

fn validate_building_preview(mut q: Query<&mut BuildingPreview, With<NetOwner>>) {
    q.iter_mut().for_each(|mut preview| {
        preview.valid = !preview.valid;
    });
}

fn draw_build_grid(mut gizmos: Gizmos, q: Query<&PlayerCursor, With<NetOwner>>) {
    let cursor = q.single();

    gizmos.grid(
        Isometry3d {
            rotation: Quat::from_axis_angle(Vec3::X, -PI * 0.5),
            translation: vec3(cursor.world_grid_pos.x, 0.01, cursor.world_grid_pos.z).into(),
        },
        UVec2::splat(16),
        Vec2::splat(1.0),
        Color::srgba(0.8, 0.8, 0.8, 0.3),
    );
}
