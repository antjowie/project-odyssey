use std::f32::consts::PI;

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::camera::PanOrbitCameraState;

/// All game systems and rules
/// 100 units is 1 meter
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerAction>::default());
        app.add_systems(PreUpdate, update_cursor);
        app.add_systems(
            Update,
            process_view_state.run_if(in_player_state(PlayerState::Viewing)),
        );
        app.add_systems(
            Update,
            process_build_state.run_if(in_player_state(PlayerState::Building)),
        );
    }
}

#[derive(Component)]
pub struct NetOwner;

#[derive(Bundle, Default)]
pub struct PlayerStateBundle {
    pub state: PlayerState,
    pub cursor: PlayerCursor,
    pub input: InputManagerBundle<PlayerAction>,
}

#[derive(Component, Default, PartialEq)]
pub enum PlayerState {
    #[default]
    Viewing,
    Building,
}

pub fn in_player_state(
    state: PlayerState,
) -> impl FnMut(Query<&PlayerState, With<NetOwner>>) -> bool {
    move |query: Query<&PlayerState, With<NetOwner>>| *query.single() == state
}

/// Component that tracks the cursor position
#[derive(Component, Default)]
pub struct PlayerCursor {
    pub screen_pos: Option<Vec2>,
    pub world_pos: Vec3,
    pub world_grid_pos: Vec3,
}

fn update_cursor(
    windows: Query<&Window>,
    cameras: Query<(&PanOrbitCameraState, &Camera, &GlobalTransform)>,
    mut q: Query<&mut PlayerCursor, With<NetOwner>>,
) {
    let window = windows.single();
    let (pan_cam_state, camera, global_transform) =
        cameras.iter().find(|(_, c, _)| c.is_active).unwrap();

    q.iter_mut().for_each(|mut cursor| {
        // Check if cursor is in window
        cursor.screen_pos = window.cursor_position();
        if let Some(ray) = cursor
            .screen_pos
            .and_then(|cursor| camera.viewport_to_world(global_transform, cursor))
        {
            // Check if cursor intersects ground
            if let Some(len) = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y)) {
                cursor.world_pos = ray.origin + ray.direction * len;
                // gizmos.sphere(cursor.position, Quat::IDENTITY, 10.0, RED);
            }
        } else {
            // Set these values to camera center, in case we do gamepad implementation
            cursor.world_pos = pan_cam_state.center;
        }
        cursor.world_grid_pos = cursor.world_pos.round();
    })
}

// This is the list of "things in the game I want to be able to do based on input"
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerAction {
    Interact,
    Cancel,
}

impl PlayerAction {
    pub fn default_player_mapping() -> InputMap<PlayerAction> {
        InputMap::default()
            .with(PlayerAction::Interact, MouseButton::Left)
            .with(PlayerAction::Cancel, KeyCode::KeyE)
            .with(PlayerAction::Cancel, KeyCode::Escape)
    }
}

fn process_view_state(
    mut q: Query<(&mut PlayerState, &ActionState<PlayerAction>), With<NetOwner>>,
) {
    let (mut state, input) = q.single_mut();

    if input.just_pressed(&PlayerAction::Interact) {
        *state = PlayerState::Building;
    }
}

fn process_build_state(
    mut gizmos: Gizmos,
    mut q: Query<(&mut PlayerState, &PlayerCursor, &ActionState<PlayerAction>), With<NetOwner>>,
) {
    let (mut state, cursor, input) = q.single_mut();

    gizmos.grid(
        Vec3::new(cursor.world_grid_pos.x, -cursor.world_grid_pos.z, 0.1),
        Quat::from_axis_angle(Vec3::X, -PI * 0.5),
        UVec2::splat(512),
        Vec2::splat(1.0),
        Color::srgba(0.8, 0.8, 0.8, 0.3),
    );

    if input.just_pressed(&PlayerAction::Interact) {
    } else if input.just_pressed(&PlayerAction::Cancel) {
        *state = PlayerState::Viewing;
    }
}
