//! Any logic or systems strongly related to gameplay and content are grouped under the game module
//!
//! These systems are made with the purpose to serve gameplay. It wouldn't make sense to use them outside
//! of this context.
//!
//! You could argue input and camera are also strongly game related, but these systems can still be used without
//! knowing about anything game related. To not bloat the root and ease reuse, the distinction is made.

use std::f32::consts::PI;
use std::fmt;

use bevy::color::palettes::tailwind::*;
use bevy::picking::pointer::PointerInteraction;
use bevy::{math::*, prelude::*, window::PrimaryWindow};

use crate::camera::*;
use crate::input::*;
use building::*;
use world::*;

pub mod building;
pub mod world;

/// All game systems and rules
/// 100 units is 1 meter
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputContextPlugin::<PlayerAction>::default());
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

        app.add_plugins(build_plugin);
        app.add_plugins(world_plugin);
    }
}

#[derive(Component)]
pub struct NetOwner;

#[derive(Component, Default, PartialEq, Clone)]
#[require(PlayerCursor, InputContext<PlayerAction>)]
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
    #[default]
    // Keep aligned with start joint
    Straight,
    // Share same angle between start and end joint
    Curve,
    // Align end joint with direction between end and start point
    Chase,
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

// TODO: Instead of one big player input object, split it in contextual input actions, so we have one for view mode and build mode
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect, PartialOrd, Ord)]
pub enum PlayerAction {
    Interact,
    Cancel,
    CancelMouse,
    Pause,
    SnapToGrid,
    Rotate,
    CounterRotate,
    SnapRotate,
    SnapCounterRotate,
    CyclePathRotateMode,
}

impl InputContextlike for PlayerAction {
    fn default_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(PlayerAction::Interact, MouseButton::Left)
            .with(PlayerAction::Cancel, KeyCode::KeyE)
            .with(PlayerAction::Cancel, KeyCode::Escape)
            .with(PlayerAction::CancelMouse, MouseButton::Right)
            .with(PlayerAction::Pause, KeyCode::Escape)
            .with(PlayerAction::SnapToGrid, KeyCode::ControlLeft)
            .with(PlayerAction::Rotate, KeyCode::KeyR)
            .with(
                PlayerAction::SnapRotate,
                ButtonlikeChord::modified(ModifierKey::Control, KeyCode::KeyR),
            )
            .with(
                PlayerAction::CounterRotate,
                ButtonlikeChord::modified(ModifierKey::Shift, KeyCode::KeyR),
            )
            .with(
                PlayerAction::SnapCounterRotate,
                ButtonlikeChord::default()
                    .with(ModifierKey::Control)
                    .with(ModifierKey::Shift)
                    .with(KeyCode::KeyR),
            )
            .with(PlayerAction::CyclePathRotateMode, KeyCode::Tab)
    }
    fn group_name() -> String {
        "Player Actions".into()
    }
}

impl fmt::Display for PlayerAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self == &PlayerAction::CyclePathRotateMode {
            write!(f, "{:?} Straight/Curve/Chase", self)
        } else {
            fmt::Debug::fmt(&self, f)
        }
    }
}

fn update_cursor(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&PanOrbitCamera, &Camera, &GlobalTransform)>,
    mut q: Query<(&mut PlayerCursor, &ActionState<PlayerAction>), With<NetOwner>>,
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

    if input.just_pressed(&PlayerAction::SnapToGrid) {
        cursor.should_snap_to_grid = !cursor.should_snap_to_grid;
    }

    if input.pressed(&PlayerAction::Rotate) {
        cursor.manual_rotation -= PI * 0.5 * time.delta_secs();
    }
    if input.pressed(&PlayerAction::CounterRotate) {
        cursor.manual_rotation += PI * 0.5 * time.delta_secs();
    }

    const SNAP_ROT: f32 = PI * 0.5;
    if input.just_pressed(&PlayerAction::SnapRotate) {
        cursor.manual_rotation = (cursor.manual_rotation / SNAP_ROT).round() * SNAP_ROT - SNAP_ROT;
    }
    if input.just_pressed(&PlayerAction::SnapCounterRotate) {
        cursor.manual_rotation = (cursor.manual_rotation / SNAP_ROT).round() * SNAP_ROT + SNAP_ROT;
    }

    if input.just_pressed(&PlayerAction::CyclePathRotateMode) {
        cursor.rotation_mode = match cursor.rotation_mode {
            PathRotationMode::Straight => PathRotationMode::Curve,
            PathRotationMode::Curve => PathRotationMode::Chase,
            PathRotationMode::Chase => PathRotationMode::Straight,
        };
        cursor.manual_rotation = 0.;
    }

    cursor.build_pos = if cursor.should_snap_to_grid {
        cursor.world_grid_pos
    } else {
        cursor.world_pos
    };
}

fn process_state_change(
    mut q: Query<(&PlayerCursor, &mut PlayerState, &ActionState<PlayerAction>), With<NetOwner>>,
    mut previews: Query<&mut BuildingPreview, With<NetOwner>>,
    mut ev_player_state: EventWriter<PlayerStateEvent>,
    mut exit: EventWriter<AppExit>,
    mut cancel_mouse_pos: Local<Vec2>,
) {
    let (cursor, mut state, input) = q.single_mut();
    let old_state = state.clone();

    match *state {
        PlayerState::Viewing => {
            if input.just_pressed(&PlayerAction::Interact) {
                *state = PlayerState::Building;
            }

            if input.just_pressed(&PlayerAction::Pause) {
                exit.send(AppExit::Success);
            }
        }
        PlayerState::Building => {
            let wants_to_place = input.just_pressed(&PlayerAction::Interact);
            previews.iter_mut().for_each(|mut preview| {
                preview.wants_to_place = wants_to_place;
            });

            if input.just_pressed(&PlayerAction::Cancel) {
                *state = PlayerState::Viewing;
            }

            if input.just_pressed(&PlayerAction::CancelMouse) {
                *cancel_mouse_pos = cursor.screen_pos.unwrap_or_default();
            } else if input.just_released(&PlayerAction::CancelMouse)
                && *cancel_mouse_pos == cursor.screen_pos.unwrap_or_default()
            {
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
