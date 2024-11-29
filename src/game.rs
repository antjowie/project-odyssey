/// All game systems and rules
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, update_cursor);
    }
}

#[derive(Bundle, Default)]
pub struct PlayerStateBundle {
    pub cursor: PlayerCursor,
}

/// Component that tracks the cursor position
#[derive(Component, Default)]
pub struct PlayerCursor {
    pub screen_position: Option<Vec2>,
    pub world_position: Option<Vec3>,
}

fn update_cursor(
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut q: Query<&mut PlayerCursor>,
) {
    let window = windows.single();
    let (camera, global_transform) = cameras.iter().find(|(c, _)| c.is_active).unwrap();

    q.iter_mut().for_each(|mut cursor| {
        // Check if cursor is in window
        cursor.world_position = None;
        cursor.screen_position = window.cursor_position();
        if let Some(ray) = cursor
            .screen_position
            .and_then(|cursor| camera.viewport_to_world(global_transform, cursor))
        {
            // Check if cursor intersects ground
            if let Some(len) = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y)) {
                cursor.world_position = Some(ray.origin + ray.direction * len);
                // gizmos.sphere(cursor.position, Quat::IDENTITY, 10.0, RED);
            }
        }
    })
}
