use std::f32::consts::PI;
use std::time::Duration;

use bevy::time::common_conditions::on_timer;
use bevy::{prelude::*, window::PrimaryWindow};
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
        app.add_systems(PreUpdate, update_cursor);
        app.add_systems(Update, (process_state_change, create_building_preview));
        // app.add_systems(
        //     Update,
        //     process_view_state_input.run_if(in_player_state(PlayerState::Viewing)),
        // );
        app.add_systems(
            Update,
            (
                snap_building_preview_to_build_pos,
                validate_building_preview.run_if(on_timer(Duration::from_secs(1))),
                draw_build_grid,
            )
                .run_if(in_player_state(PlayerState::Building)),
        );
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

#[derive(Event)]
pub struct PlayerStateEvent {
    new_state: PlayerState,
    old_state: PlayerState,
}

/// Component that tracks the cursor position
#[derive(Component, Default)]
pub struct PlayerCursor {
    pub screen_pos: Option<Vec2>,
    pub should_snap_to_grid: bool,
    // Can be world or grid pos based on user desire
    pub build_pos: Vec3,
    pub world_pos: Vec3,
    pub world_grid_pos: Vec3,
}

fn update_cursor(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&PanOrbitCameraState, &Camera, &GlobalTransform)>,
    mut q: Query<(&mut PlayerCursor, &ActionState<PlayerInput>), With<NetOwner>>,
) {
    let window = windows.single();
    let (pan_cam_state, camera, global_transform) =
        cameras.iter().find(|(_, c, _)| c.is_active).unwrap();

    q.iter_mut().for_each(|(mut cursor, input)| {
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

        if input.just_pressed(&PlayerInput::SnapToGrid) {
            cursor.should_snap_to_grid = !cursor.should_snap_to_grid;
        }

        cursor.build_pos = if cursor.should_snap_to_grid {
            cursor.world_grid_pos
        } else {
            cursor.world_pos
        };
    })
}

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerInput {
    Interact,
    Cancel,
    Pause,
    SnapToGrid,
}

impl PlayerInput {
    pub fn default_player_mapping() -> InputMap<PlayerInput> {
        InputMap::default()
            .with(PlayerInput::Interact, MouseButton::Left)
            .with(PlayerInput::Cancel, KeyCode::KeyE)
            .with(PlayerInput::Cancel, KeyCode::Escape)
            .with(PlayerInput::Pause, KeyCode::Escape)
            .with(PlayerInput::SnapToGrid, KeyCode::ShiftLeft)
    }
}

fn process_state_change(
    mut c: Commands,
    mut q: Query<(&mut PlayerState, &ActionState<PlayerInput>), With<NetOwner>>,
    preview: Query<Entity, (With<NetOwner>, With<BuildingPreview>)>,
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
            if input.just_pressed(&PlayerInput::Interact) {
                preview.into_iter().for_each(|e| {
                    c.entity(e).insert(PlaceBuildingPreview);
                });
            } else if input.just_pressed(&PlayerInput::Cancel) {
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

fn create_building_preview(
    q: Query<Entity, (With<NetOwner>, With<BuildingPreview>)>,
    mut c: Commands,
    mut event: EventReader<PlayerStateEvent>,
) {
    for e in event.read() {
        if e.new_state == PlayerState::Building && e.old_state == PlayerState::Viewing {
            c.add(SpawnRail {
                is_preview: true,
                ..default()
            });
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
        Vec3::new(cursor.world_grid_pos.x, -cursor.world_grid_pos.z, 0.01),
        Quat::from_axis_angle(Vec3::X, -PI * 0.5),
        UVec2::splat(512),
        Vec2::splat(1.0),
        Color::srgba(0.8, 0.8, 0.8, 0.3),
    );
}
