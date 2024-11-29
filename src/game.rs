use std::f32::consts::PI;

use bevy::{color::palettes::css::GREY, prelude::*};
use leafwing_input_manager::prelude::*;

use crate::camera::PanOrbitCameraState;

/// All game systems and rules
/// 100 units is 1 meter
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, update_cursor);
        app.add_systems(Update, process_build_mode);
    }
}

#[derive(Component)]
pub struct NetOwner;

#[derive(Bundle, Default)]
pub struct PlayerStateBundle {
    pub cursor: PlayerCursor,
    pub build_actions: ActionState<BuildAction>,
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
pub enum BuildAction {
    Build,
}

fn process_build_mode(
    mut gizmos: Gizmos,
    cursor: Query<&PlayerCursor, With<NetOwner>>,
    q: Query<&ActionState<BuildAction>>,
) {
    let cursor = cursor.single();

    return;
    q.iter().for_each(|input| {
        gizmos.grid(
            Vec3::new(cursor.world_grid_pos.x, -cursor.world_grid_pos.z, 0.1),
            Quat::from_axis_angle(Vec3::X, -PI * 0.5),
            UVec2::splat(512),
            Vec2::splat(1.0),
            GREY,
        );
    });
}
